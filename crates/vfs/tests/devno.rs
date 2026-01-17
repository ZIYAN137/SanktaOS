use vfs::{
    blkdev_major, chrdev_major,
    dev::{major, makedev, minor},
    get_blkdev_index,
};

#[test]
fn test_makedev_major_minor() {
    let dev = makedev(8, 1);
    assert_eq!(major(dev), 8);
    assert_eq!(minor(dev), 1);
}

#[test]
fn test_makedev_zero() {
    let dev = makedev(0, 0);
    assert_eq!(major(dev), 0);
    assert_eq!(minor(dev), 0);
}

#[test]
fn test_makedev_large_numbers() {
    let dev = makedev(255, 255);
    assert_eq!(major(dev), 255);
    assert_eq!(minor(dev), 255);
}

#[test]
fn test_makedev_roundtrip() {
    for maj in [0, 1, 8, 10, 100, 255] {
        for min in [0, 1, 16, 100, 255] {
            let dev = makedev(maj, min);
            assert_eq!(major(dev), maj);
            assert_eq!(minor(dev), min);
        }
    }
}

#[test]
fn test_blkdev_major_constants() {
    assert_eq!(blkdev_major::LOOP, 7);
    assert_eq!(blkdev_major::SCSI_DISK, 8);
    assert_eq!(blkdev_major::VIRTIO_BLK, 254);
}

#[test]
fn test_chrdev_major_constants() {
    assert_eq!(chrdev_major::MEM, 1);
    assert_eq!(chrdev_major::TTY, 4);
    assert_eq!(chrdev_major::CONSOLE, 5);
    assert_eq!(chrdev_major::INPUT, 13);
}

#[test]
fn test_get_blkdev_index_virtio_blk() {
    assert_eq!(
        get_blkdev_index(makedev(blkdev_major::VIRTIO_BLK, 0)),
        Some(0)
    );
    assert_eq!(
        get_blkdev_index(makedev(blkdev_major::VIRTIO_BLK, 3)),
        Some(3)
    );
}

#[test]
fn test_get_blkdev_index_scsi_disk() {
    // Each disk occupies 16 minors.
    assert_eq!(
        get_blkdev_index(makedev(blkdev_major::SCSI_DISK, 0)),
        Some(0)
    );
    assert_eq!(
        get_blkdev_index(makedev(blkdev_major::SCSI_DISK, 15)),
        Some(0)
    );
    assert_eq!(
        get_blkdev_index(makedev(blkdev_major::SCSI_DISK, 16)),
        Some(1)
    );
    assert_eq!(
        get_blkdev_index(makedev(blkdev_major::SCSI_DISK, 32)),
        Some(2)
    );
}

#[test]
fn test_get_blkdev_index_loop_is_none() {
    assert_eq!(get_blkdev_index(makedev(blkdev_major::LOOP, 0)), None);
}

#[test]
fn test_get_blkdev_index_unknown_is_none() {
    assert_eq!(get_blkdev_index(makedev(9, 0)), None);
}

#[test]
fn test_devno_unique() {
    let dev1 = makedev(1, 0);
    let dev2 = makedev(1, 1);
    let dev3 = makedev(2, 0);

    assert_ne!(dev1, dev2);
    assert_ne!(dev1, dev3);
    assert_ne!(dev2, dev3);
}

#[test]
fn test_major_minor_extraction_boundaries() {
    let dev = makedev(255, 0);
    assert_eq!(major(dev), 255);
    assert_eq!(minor(dev), 0);

    let dev = makedev(0, 255);
    assert_eq!(major(dev), 0);
    assert_eq!(minor(dev), 255);
}

#[test]
fn test_devno_consistency() {
    let dev1 = makedev(10, 20);
    let dev2 = makedev(10, 20);
    assert_eq!(dev1, dev2);
    assert_eq!(major(dev1), major(dev2));
    assert_eq!(minor(dev1), minor(dev2));
}
