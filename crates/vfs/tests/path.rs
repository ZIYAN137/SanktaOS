use vfs::{FsError, PathComponent, normalize_path, parse_path, split_path};

#[test]
fn test_normalize_path_absolute() {
    assert_eq!(normalize_path("/foo/bar"), "/foo/bar");
    assert_eq!(normalize_path("/"), "/");
    assert_eq!(normalize_path("///foo///bar///"), "/foo/bar");
}

#[test]
fn test_normalize_path_current() {
    assert_eq!(normalize_path("/foo/./bar"), "/foo/bar");
    assert_eq!(normalize_path("./foo"), "foo");
    assert_eq!(normalize_path("."), ".");
}

#[test]
fn test_normalize_path_parent() {
    assert_eq!(normalize_path("/foo/bar/.."), "/foo");
    assert_eq!(normalize_path("/foo/../bar"), "/bar");
    assert_eq!(normalize_path("/.."), "/");
    assert_eq!(normalize_path("/../.."), "/");
}

#[test]
fn test_normalize_path_relative() {
    assert_eq!(normalize_path("foo/bar"), "foo/bar");
    assert_eq!(normalize_path("foo/../bar"), "bar");
    assert_eq!(normalize_path("../foo"), "../foo");
    assert_eq!(normalize_path("../../foo"), "../../foo");
}

#[test]
fn test_split_path_absolute() {
    assert_eq!(
        split_path("/foo/bar.txt").unwrap(),
        ("/foo".to_string(), "bar.txt".to_string())
    );
    assert_eq!(
        split_path("/hello").unwrap(),
        ("/".to_string(), "hello".to_string())
    );
}

#[test]
fn test_split_path_relative() {
    assert_eq!(
        split_path("foo/bar.txt").unwrap(),
        ("foo".to_string(), "bar.txt".to_string())
    );
    assert_eq!(
        split_path("hello.txt").unwrap(),
        (".".to_string(), "hello.txt".to_string())
    );
}

#[test]
fn test_normalize_path_empty() {
    assert_eq!(normalize_path(""), ".");
}

#[test]
fn test_normalize_path_complex() {
    assert_eq!(normalize_path("/foo/./bar/../baz/./qux/.."), "/foo/baz");
    assert_eq!(normalize_path("foo/bar/../../baz"), "baz");
}

#[test]
fn test_split_path_trailing_slash() {
    let result = split_path("/foo/bar/");
    assert!(matches!(result, Err(FsError::InvalidArgument)));
}

#[test]
fn test_split_path_multiple_slashes() {
    assert_eq!(
        split_path("///foo///bar.txt").unwrap(),
        ("/foo".to_string(), "bar.txt".to_string())
    );
}

#[test]
fn test_parse_path_components() {
    let components = parse_path("/foo/bar");
    assert_eq!(components.len(), 3);
    assert_eq!(components[0], PathComponent::Root);
    assert_eq!(components[1], PathComponent::Normal("foo".to_string()));
    assert_eq!(components[2], PathComponent::Normal("bar".to_string()));

    let components = parse_path("foo/./bar/../baz");
    assert_eq!(components.len(), 5);
    assert_eq!(components[0], PathComponent::Normal("foo".to_string()));
    assert_eq!(components[1], PathComponent::Current);
    assert_eq!(components[2], PathComponent::Normal("bar".to_string()));
    assert_eq!(components[3], PathComponent::Parent);
    assert_eq!(components[4], PathComponent::Normal("baz".to_string()));
}

#[test]
fn test_normalize_path_root_parent() {
    assert_eq!(normalize_path("/foo/.."), "/");
    assert_eq!(normalize_path("/foo/bar/../.."), "/");
}

