pub trait Idx {
    fn idx(self) -> usize;
}

impl<T> Idx for T
where
    usize: TryFrom<T>,
    <usize as TryFrom<T>>::Error: std::fmt::Debug,
{
    fn idx(self) -> usize {
        let r: Result<usize, <usize as TryFrom<T>>::Error> = usize::try_from(self);
        r.expect("Index can be converted to usize")
    }
}
