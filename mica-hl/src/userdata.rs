use std::any::Any;
use std::cell::{Cell, UnsafeCell};
use std::marker::PhantomData;
use std::rc::Rc;

use mica_language::bytecode::DispatchTable;
use mica_language::value;

use crate::Error;

/// Marker trait for all user data types.
///
/// Due to limitations in Rust's type system each user-defined type must implement this.
pub trait UserData: Any {}

/// A type. This contains no data besides the type dtable and may be used to construct [`Object`]s.
pub struct Type<T> {
   dtable: Rc<DispatchTable>,
   _data: PhantomData<T>,
}

impl<T> Type<T> {
   pub(crate) fn new(dtable: Rc<DispatchTable>) -> Self {
      Self {
         dtable,
         _data: PhantomData,
      }
   }
}

impl<T> value::UserData for Type<T>
where
   T: Any,
{
   fn dtable(&self) -> &DispatchTable {
      &self.dtable
   }
}

/// A constructor of objects of type `T`.
pub trait ObjectConstructor<T> {
   /// Constructs an object.
   fn construct(&self, instance: T) -> Object<T>;
}

/// Represents a custom-typed value stored inside a VM.
///
/// Built-in types such as [`f64`] shouldn't be used as the `T` of an `Object`, because they are
/// can be represented by values inherently. `Object` however may be used for binding types that are
/// not directly supported by the VM, like [`std::fs::File`].
pub struct Object<T> {
   pub(crate) dtable: Rc<DispatchTable>,
   // The functionality of the RefCell unfortunately has to be replicated because we need unsafe
   // guards that the standard RefCell doesn't provide.
   shared_borrows: Cell<usize>,
   borrowed_mutably: Cell<bool>,
   data: UnsafeCell<T>,
}

impl<T> Object<T> {
   pub(crate) fn new(dtable: Rc<DispatchTable>, data: T) -> Self {
      Self {
         dtable,
         shared_borrows: Cell::new(0),
         borrowed_mutably: Cell::new(false),
         data: UnsafeCell::new(data),
      }
   }

   /// Borrows the object as immutably using an unsafe guard.
   #[doc(hidden)]
   pub unsafe fn unsafe_borrow(&self) -> Result<(&T, UnsafeRefGuard<T>), Error> {
      if self.borrowed_mutably.get() {
         return Err(Error::ReentrantMutableBorrow);
      }
      self.shared_borrows.set(self.shared_borrows.get() + 1);
      let reference = &*self.data.get();
      Ok((
         reference,
         UnsafeRefGuard {
            object: self as *const _,
         },
      ))
   }

   /// Borrows the object mutably using an unsafe guard.
   #[doc(hidden)]
   pub unsafe fn unsafe_borrow_mut(&self) -> Result<(&mut T, UnsafeMutGuard<T>), Error> {
      if self.shared_borrows.get() > 0 {
         return Err(Error::ReentrantMutableBorrow);
      }
      self.borrowed_mutably.set(true);
      let reference = &mut *self.data.get();
      Ok((
         reference,
         UnsafeMutGuard {
            object: self as *const _,
         },
      ))
   }
}

impl<T> value::UserData for Object<T>
where
   T: Any,
{
   fn dtable(&self) -> &DispatchTable {
      &self.dtable
   }
}

/// An _unsafe_ guard for a `&T` borrowed from an `Object<T>`.
///
/// This is an **unsafe** guard because it must not outlive the `Object<T>` it guards, but that
/// is not guaranteed using the borrow checker. This type is needed because generic associated types
/// haven't been stabilized yet.
#[doc(hidden)]
pub struct UnsafeRefGuard<T> {
   object: *const Object<T>,
}

impl<T> Drop for UnsafeRefGuard<T> {
   fn drop(&mut self) {
      unsafe {
         let object = &*self.object;
         object.shared_borrows.set(object.shared_borrows.get() - 1);
      }
   }
}

/// An _unsafe_ guard for a `&mut T` borrowed from an `Object<T>`.
///
/// This is an **unsafe** guard because it must not outlive the `Object<T>` it guards, but that
/// is not guaranteed using the borrow checker. This type is needed because generic associated types
/// haven't been stabilized yet.
#[doc(hidden)]
pub struct UnsafeMutGuard<T> {
   object: *const Object<T>,
}

impl<T> Drop for UnsafeMutGuard<T> {
   fn drop(&mut self) {
      unsafe {
         let object = &*self.object;
         object.borrowed_mutably.set(false);
      }
   }
}