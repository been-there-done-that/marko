# Image-heavy fixture

Six 4000 × 3000 pictures: four PNG and two JPEG. Each PNG decodes to 48 MB of RGBA, so opening
this document exercises the per-image cap, the cache eviction, and the one-decode-at-a-time lock.
The JPEGs take the scaled DCT path and never allocate the full-resolution buffer.

The pictures are solid colours to keep the files tiny; only their dimensions matter for the decode
measurement. Regenerate them with `python3 scripts/make-image-fixtures.py`.

![solid red](images/photo-1.png)

A little text between the pictures, so the document is more than an image list.

![solid green](images/photo-2.png)

![solid blue](images/photo-3.png)

![solid amber](images/photo-4.png)

![solid purple](images/photo-5.jpg)

![solid teal](images/photo-6.jpg)

The end. Scrolling here queues the next window only when the reader gets close to it.
