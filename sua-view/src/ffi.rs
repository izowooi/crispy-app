use crate::Book;
use std::ffi::{CStr, CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;

// Fixed-width integers keep the C ABI identical in Swift and .NET (whose bool
// marshalling defaults differ). Pure reading rules require no book lock.
#[unsafe(no_mangle)]
pub extern "C" fn sua_page_is_landscape(width: f64, height: f64) -> u8 {
    u8::from(crate::reading::is_landscape(width, height))
}

#[unsafe(no_mangle)]
pub extern "C" fn sua_can_pair(
    index: usize,
    end: usize,
    spread: u8,
    cover: u8,
    first_landscape: u8,
    second_landscape: u8,
) -> u8 {
    u8::from(crate::reading::can_pair(
        index,
        end,
        spread != 0,
        cover != 0,
        first_landscape != 0,
        second_landscape != 0,
    ))
}

#[unsafe(no_mangle)]
pub extern "C" fn sua_next_start(index: usize, count: usize, displayed: usize) -> usize {
    crate::reading::next_start(index, count, displayed)
}

#[unsafe(no_mangle)]
pub extern "C" fn sua_previous_start(
    index: usize,
    spread: u8,
    cover: u8,
    previous_landscape: i32,
    earlier_landscape: i32,
) -> usize {
    crate::reading::previous_start(
        index,
        spread != 0,
        cover != 0,
        previous_landscape,
        earlier_landscape,
    )
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sua_path_is_image(path: *const c_char) -> u8 {
    if path.is_null() {
        return 0;
    }
    u8::from(
        unsafe { CStr::from_ptr(path) }
            .to_str()
            .is_ok_and(crate::is_image),
    )
}

fn string(text: &str) -> *mut c_char {
    CString::new(text.replace('\0', "�"))
        .map(CString::into_raw)
        .unwrap_or(ptr::null_mut())
}

unsafe fn set_error(out: *mut *mut c_char, message: Option<&str>) {
    if !out.is_null() {
        unsafe {
            *out = message.map(string).unwrap_or(ptr::null_mut());
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sua_book_open(path: *const c_char, error: *mut *mut c_char) -> *mut Book {
    unsafe {
        set_error(error, None);
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        if path.is_null() {
            return Err("No book path was supplied.".into());
        }
        let path = unsafe { CStr::from_ptr(path) }
            .to_str()
            .map_err(|_| "Book path must be valid UTF-8")?;
        Book::open(path)
    }));
    match result {
        Ok(Ok(book)) => Box::into_raw(Box::new(book)),
        Ok(Err(message)) => {
            unsafe {
                set_error(error, Some(&message));
            }
            ptr::null_mut()
        }
        Err(_) => {
            unsafe {
                set_error(
                    error,
                    Some("The book reader encountered an unexpected error."),
                );
            }
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sua_book_count(book: *const Book) -> usize {
    if book.is_null() {
        0
    } else {
        unsafe { (*book).len() }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sua_book_initial_page(book: *const Book) -> usize {
    if book.is_null() {
        0
    } else {
        unsafe { (*book).initial_page() }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sua_book_page_name(book: *const Book, index: usize) -> *mut c_char {
    if book.is_null() {
        return ptr::null_mut();
    }
    catch_unwind(AssertUnwindSafe(|| {
        unsafe { (*book).page_name(index) }
            .map(string)
            .unwrap_or(ptr::null_mut())
    }))
    .unwrap_or(ptr::null_mut())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sua_book_read_page(
    book: *mut Book,
    index: usize,
    length: *mut usize,
    error: *mut *mut c_char,
) -> *mut u8 {
    unsafe {
        set_error(error, None);
    }
    if !length.is_null() {
        unsafe {
            *length = 0;
        }
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        if book.is_null() {
            return Err("No book is open.".into());
        }
        if length.is_null() {
            return Err("A byte-length output is required.".into());
        }
        unsafe { (*book).read_page(index) }
    }));
    match result {
        Ok(Ok(bytes)) => {
            unsafe {
                *length = bytes.len();
            }
            Box::into_raw(bytes.into_boxed_slice()).cast::<u8>()
        }
        Ok(Err(message)) => {
            unsafe {
                set_error(error, Some(&message));
            }
            ptr::null_mut()
        }
        Err(_) => {
            unsafe {
                set_error(
                    error,
                    Some("The page reader encountered an unexpected error."),
                );
            }
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sua_bytes_free(bytes: *mut u8, length: usize) {
    if !bytes.is_null() {
        unsafe {
            drop(Box::from_raw(ptr::slice_from_raw_parts_mut(bytes, length)));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sua_string_free(text: *mut c_char) {
    if !text.is_null() {
        unsafe {
            drop(CString::from_raw(text));
        }
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn sua_book_close(book: *mut Book) {
    if !book.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| unsafe {
            drop(Box::from_raw(book));
        }));
    }
}
