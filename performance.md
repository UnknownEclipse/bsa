# Performance

Performance of reading and writing archives is heavily dependent on two factors:

1. Disk I/O speed
2. Compression and decompression speed.

## Disk I/O: Reading

To improve disk I/O throughput, the best strategy is to limit random accesses and
small reads as much as possible. Reads should be as sequential as possible, and
large block reads should be preferred over smaller ones. Unfortunately, this strategy
limits I/O _on an archive_ to single-threaded operations.

One way to avoid this is to perform reads and writes to the archive in one thread,
but use concurrency for reads and writes to individual files.

For example, when extracting, we will loop through the archive sequentially, extracting
any data into a buffer. Then, we pass that buffer to a threadpool to be written to a
file. The reverse is true for packing: we use multiple concurrent reads from individual
files and a single writer thread to write the archive.

## Compression

Compression is heavily CPU bound. This can present a problem, as in a single-threaded
read->decompress->write extraction operation both the reader and writers are forced
to wait for the decompressor, time when they could be performing IO. We remedy this
in a similar way to pack/unpack: Read data into memory, and split each job onto a
threadpool.

## Conclusion

The optimal solution that has been found so far is as follows: The archive is read
sequentially on one thread. As soon as the data has been read into memory, that data is
submitted to a threadpool for decompression and to be written to a file.

## Outstanding Issues

It may prove beneficial to split the decompression and write operations.
If our threadpool is waiting for I/O, it may waste time that could have been used for
more decompression. A more optimal solution might be as follows: The single threaded
reader thread is still here. Buffers are passed to a threadpool for decompression.
Then, instead of file writing also occurring in the threadpool, we have an asynchronous
writer thread (potentially backed by Tokio) that writes the decompressed buffers
provided by the threadpool.

### Notes

Unfortunately, an async file writing system is not available on every platform.
Modern linux has `io_uring`, and Windows has IOCP. That's it.
