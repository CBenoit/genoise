use genoise::local;
use genoise::{Co, GeneratorFlavor};

// Such an iterator would be hard to write by hand in today’s stable Rust
async fn deep_iterator_impl<'a, T, F>(mut co: Co<'_, &'a T, (), F>, slice_of_slices: &[&'a [T]])
where
    F: GeneratorFlavor,
{
    if slice_of_slices.is_empty() {
        return;
    }

    for slice in slice_of_slices {
        for value in *slice {
            co.suspend(value).await;
        }
    }
}

// Thin wrapper around a generator
pub struct DeepIterator<'a, T> {
    // Implementation detail
    inner: local::Gn<'a, 'a, &'a T, (), ()>,
}

impl<'a, T> DeepIterator<'a, T> {
    pub fn new(slice_of_slices: &'a [&'a [T]]) -> Self {
        let inner = local::Gn::new(|co| deep_iterator_impl(co, slice_of_slices));
        Self { inner }
    }
}

impl<'a, T> IntoIterator for DeepIterator<'a, T> {
    type Item = &'a T;

    type IntoIter = local::Gn<'a, 'a, &'a T, (), ()>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner
    }
}

fn main() {
    let it = DeepIterator::new(&[&[1, 2, 3], &[4, 5, 6], &[7, 8, 9]]);

    for value in it {
        println!("{value}");
    }
}
