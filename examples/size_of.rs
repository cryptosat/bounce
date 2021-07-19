use std::mem;

#[allow(dead_code)]
struct Point {
    x: i32,
    y: i32,
}

struct Points {
    inner: Vec<Point>,
}

fn main() {
    assert_eq!(8, mem::size_of::<Point>());
    assert_eq!(24, mem::size_of::<Points>());

    let mut pts = Points { inner: vec![] };
    // The Points struct has only one field inner, which is a vector of Points.
    assert_eq!(24, mem::size_of_val(&pts));
    // A Vec is a triplet of (pointer, capacity, length), which are 8 bytes each.
    assert_eq!(24, mem::size_of_val(&pts.inner));
    // The inner needs to be dereferenced to get the actual size of the vector.
    assert_eq!(0, mem::size_of_val(&*pts.inner));
    pts.inner.push(Point { x: 1, y: 2 });
    assert_eq!(8, mem::size_of_val(&*pts.inner));
}
