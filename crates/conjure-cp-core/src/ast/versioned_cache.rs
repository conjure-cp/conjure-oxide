use std::sync::Mutex;

/// A lazily populated cache whose entries are valid for one external generation.
#[derive(Debug)]
pub(crate) struct VersionedCache<T> {
    value: Mutex<Option<(u64, T)>>,
}

impl<T> VersionedCache<T> {
    pub(crate) fn new() -> Self {
        Self {
            value: Mutex::new(None),
        }
    }

    pub(crate) fn clear(&self) {
        *self.value.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

impl<T: Clone> VersionedCache<T> {
    /// Returns the cached value for the current generation, computing it on a cache miss.
    ///
    /// If the generation changes during computation, the result is discarded and recomputed so
    /// a value is never cached against a generation it may not represent.
    pub(crate) fn get_or_init(
        &self,
        generation: impl Fn() -> u64,
        mut compute: impl FnMut(u64) -> T,
    ) -> T {
        loop {
            let generation_before = generation();
            let mut cached = self.value.lock().unwrap_or_else(|e| e.into_inner());

            if let Some((cached_generation, value)) = cached.as_ref()
                && *cached_generation == generation_before
            {
                return value.clone();
            }

            let computed = compute(generation_before);
            let generation_after = generation();
            if generation_before == generation_after {
                *cached = Some((generation_after, computed.clone()));
                return computed;
            }
        }
    }
}

impl<T> Default for VersionedCache<T> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn retries_when_generation_changes_during_computation() {
        let generation = Cell::new(1);
        let computations = Cell::new(0);
        let cache = VersionedCache::new();

        let value = cache.get_or_init(
            || generation.get(),
            |_| {
                let attempt = computations.get() + 1;
                computations.set(attempt);
                if attempt == 1 {
                    generation.set(2);
                }
                attempt
            },
        );

        assert_eq!(value, 2);
        assert_eq!(computations.get(), 2);
        assert_eq!(cache.get_or_init(|| generation.get(), |_| 3), 2);
    }
}
