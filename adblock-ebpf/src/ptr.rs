//! 边界安全的报文访问原语。
//!
//! 读侧：从报文（data..data_end）逐字段读，显式边界检查。
//! 写侧：写入 `&mut MaybeUninit<T>` —— 典型是 ringbuf 槽位。
//!      eBPF verifier 对 ringbuf 允许 variable offset 写，对栈只允许 fixed offset 写；
//!      因此 parse 阶段的逐字节写入必须直接落到 ringbuf，不能先进栈数组。

use core::mem::{MaybeUninit, size_of};

#[inline(always)]
fn ptr_at<T>(data: usize, offset: usize, data_end: usize) -> Result<*const T, ()> {
    if data + offset + size_of::<T>() > data_end {
        return Err(());
    }
    Ok((data + offset) as *const T)
}

#[inline(always)]
pub fn read_u8(data: usize, offset: usize, data_end: usize) -> Result<u8, ()> {
    let p = ptr_at::<u8>(data, offset, data_end)?;
    // SAFETY: ptr_at 已确认 data+offset .. data+offset+1 在界内
    Ok(unsafe { *p })
}

#[inline(always)]
pub fn read_u16_be(data: usize, offset: usize, data_end: usize) -> Result<u16, ()> {
    let hi = read_u8(data, offset, data_end)?;
    let lo = read_u8(data, offset + 1, data_end)?;
    Ok(((hi as u16) << 8) | (lo as u16))
}

/// 向 `MaybeUninit<T>` 的 byte_offset 处写单字节。
/// 目标通常是 ringbuf 槽位 —— verifier 允许 variable offset 写。
#[inline(always)]
pub fn write_u8_in<T>(m: &mut MaybeUninit<T>, byte_offset: usize, v: u8) -> Result<(), ()> {
    if byte_offset >= size_of::<T>() {
        return Err(());
    }
    let base = m.as_mut_ptr() as *mut u8;
    // SAFETY: byte_offset < size_of::<T>()，m 借用期间有效
    unsafe {
        *base.add(byte_offset) = v;
    }
    Ok(())
}

/// 向 `MaybeUninit<T>` 的 byte_offset 处写原生序 u16（用户态用 from_ne_bytes 读）。
#[inline(always)]
pub fn write_u16_in<T>(m: &mut MaybeUninit<T>, byte_offset: usize, v: u16) -> Result<(), ()> {
    if byte_offset + 2 > size_of::<T>() {
        return Err(());
    }
    let bytes = v.to_ne_bytes();
    let base = m.as_mut_ptr() as *mut u8;
    // SAFETY: byte_offset + 2 <= size_of::<T>()
    unsafe {
        *base.add(byte_offset) = bytes[0];
        *base.add(byte_offset + 1) = bytes[1];
    }
    Ok(())
}

/// 向 `MaybeUninit<T>` 的 byte_offset 处写一段字节。
/// src 是 read 侧的 slice（读越界走 get -> None，不 panic）。
#[inline(always)]
pub fn write_bytes_in<T>(m: &mut MaybeUninit<T>, byte_offset: usize, src: &[u8]) -> Result<(), ()> {
    let end = match byte_offset.checked_add(src.len()) {
        Some(e) => e,
        None => return Err(()),
    };
    if end > size_of::<T>() {
        return Err(());
    }
    let base = m.as_mut_ptr() as *mut u8;
    let mut i = 0;
    while i < src.len() {
        let b = match src.get(i).copied() {
            Some(b) => b,
            None => return Err(()),
        };
        // SAFETY: byte_offset + i < end <= size_of::<T>()
        unsafe {
            *base.add(byte_offset + i) = b;
        }
        i += 1;
    }
    Ok(())
}
