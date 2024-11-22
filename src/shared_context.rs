use parking_lot::{Mutex, MutexGuard};
use std::sync::Arc;

/// A thread-safe shared context that wraps a value of type `T` using an `Arc<Mutex<T>>`.
///
/// This allows multiple threads to have shared ownership of the value and safely access
/// or modify it.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use parking_lot::Mutex;
/// use command_history::shared_context::SharedContext;
///
/// let context = SharedContext::new(5);
/// {
///     let mut value = context.lock();
///     *value += 1;
/// }
/// assert_eq!(*context.lock(), 6);
/// ```
///
/// # Type Parameters
///
/// * `T` - The type of the value to be shared.
///
/// # Methods
///
/// * `new(value: T) -> Self` - Creates a new `SharedContext` with the given value. The value is wrapped in an `Arc<Mutex<T>>`.
/// * `lock(&self) -> MutexGuard<'_, T>` - Locks the mutex and returns a guard that allows access to the value. Blocks if the mutex is already locked.
/// * `try_lock(&self) -> Option<MutexGuard<'_, T>>` - Tries to lock the mutex and returns a guard if successful. Returns `None` if the mutex is already locked.
/// * `into_inner(self) -> T` - Consumes the `SharedContext` and returns the inner value. Panics if there are multiple references to the `SharedContext`.
/// * `modify<F>(&self, f: F)` - Modifies the value using the given closure. The value is locked during the call.
///
/// # Traits
///
/// * `Clone` - Allows cloning the `SharedContext`, which will share the same underlying value.
/// * `From<Arc<Mutex<T>>>` - Allows creating a `SharedContext` from an existing `Arc<Mutex<T>>`.
/// * `AsRef<Arc<Mutex<T>>>` - Allows getting a reference to the underlying `Arc<Mutex<T>>`.
/// * `Default` - Allows creating a default `SharedContext` with a default value of `T`.
/// * `Debug` - Allows debugging the `SharedContext`. The value is locked during the call.
pub struct SharedContext<T> {
    inner: Arc<Mutex<T>>,
}

impl<T> SharedContext<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(Mutex::new(value)),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, T> {
        self.inner.lock()
    }

    #[allow(clippy::must_use_candidate)]
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        self.inner.try_lock()
    }

    /// Consumes the `SharedContext` and returns the inner value.
    ///
    /// # Panics
    ///
    /// Panics if there are multiple references to the `SharedContext`.
    #[allow(clippy::must_use_candidate)]
    pub fn into_inner(self) -> T {
        Arc::try_unwrap(self.inner)
            .ok()
            .expect("Multiple references to SharedContext exist")
            .into_inner()
    }

    pub fn modify<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        let mut value = self.lock();
        f(&mut *value);
    }
}

impl<T> Clone for SharedContext<T> {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

impl<T> Default for SharedContext<T>
where
    T: Default,
{
    fn default() -> Self {
        Self::new(T::default())
    }
}

impl<T> std::fmt::Debug for SharedContext<T>
where
    T: std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.inner.try_lock() {
            Some(value) => write!(f, "SharedContext({:?})", *value),
            None => write!(f, "SharedContext(<locked>)"),
        }
    }
}

impl<T> From<Arc<Mutex<T>>> for SharedContext<T> {
    fn from(arc: Arc<Mutex<T>>) -> Self {
        Self { inner: arc }
    }
}

impl<T> AsRef<Arc<Mutex<T>>> for SharedContext<T> {
    fn as_ref(&self) -> &Arc<Mutex<T>> {
        &self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new() {
        let context = SharedContext::new(5);
        assert_eq!(*context.lock(), 5);
    }

    #[test]
    fn test_lock() {
        let context = SharedContext::new(10);
        {
            let mut value = context.lock();
            *value += 5;
        }
        assert_eq!(*context.lock(), 15);
    }

    #[test]
    fn test_clone() {
        let context = SharedContext::new(20);
        let cloned_context = context.clone();
        {
            let mut value = cloned_context.lock();
            *value += 10;
        }
        assert_eq!(*context.lock(), 30);
        assert_eq!(*cloned_context.lock(), 30);
    }

    #[test]
    fn test_default() {
        type DefaultType = i32;
        let context: SharedContext<DefaultType> = SharedContext::default();
        assert_eq!(*context.lock(), DefaultType::default());
    }

    #[test]
    fn test_from_arc_mutex() {
        let arc = Arc::new(Mutex::new(50));
        let context: SharedContext<i32> = SharedContext::from(arc.clone());
        assert_eq!(*context.lock(), 50);
        {
            let mut value = arc.lock();
            *value += 25;
        }
        assert_eq!(*context.lock(), 75);
    }

    #[test]
    fn test_as_ref() {
        let context = SharedContext::new(100);
        let arc_ref: &Arc<Mutex<i32>> = context.as_ref();
        assert_eq!(*arc_ref.lock(), 100);
    }

    #[test]
    fn test_into_inner() {
        let context = SharedContext::new(100);
        let value = context.into_inner();
        assert_eq!(value, 100);
    }

    #[test]
    fn test_modify() {
        let context = SharedContext::new(100);
        context.modify(|value| *value += 10);
        assert_eq!(*context.lock(), 110);
    }

    #[test]
    #[should_panic(expected = "Multiple references to SharedContext exist")]
    fn test_into_inner_should_panic() {
        let context = SharedContext::new(100);
        let _cloned_context = context.clone();
        context.into_inner();
    }

    #[test]
    fn test_try_lock() {
        let context = SharedContext::new(5);

        // Case 1: Successfully acquire the lock
        {
            let guard = context.try_lock();
            assert!(
                guard.is_some(),
                "Expected try_lock to succeed but it failed"
            );
            let guard = guard.unwrap();
            assert_eq!(*guard, 5);
        }

        // Case 2: Failing to acquire the lock when it's already held
        {
            let _guard = context.lock(); // This will hold the lock

            assert!(
                context.try_lock().is_none(),
                "Expected try_lock to fail but it succeeded"
            );
        }

        // Case 3: Release the lock and then successfully re-acquire it
        // After _guard goes out of scope, the lock should be released
        let guard = context.try_lock();
        assert!(guard.is_some());
        assert_eq!(*guard.unwrap(), 5);
    }

    #[test]
    fn test_debug() {
        let context = SharedContext::new(5);
        assert_eq!(format!("{:?}", context), "SharedContext(5)");
        let _guard = context.lock();
        assert_eq!(format!("{:?}", context), "SharedContext(<locked>)");
    }
}