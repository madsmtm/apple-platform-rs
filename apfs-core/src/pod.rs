// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

//! Plain old data.

use crate::error::{ApfsError, Result};

pub fn parse_le_u16(offset: &mut usize, data: &[u8]) -> Result<u16> {
    let end = offset.checked_add(2).ok_or(ApfsError::InputTooSmall)?;
    let buf: [u8; 2] = data
        .get(*offset..end)
        .ok_or(ApfsError::InputTooSmall)?
        .try_into()
        .expect("buffer coercion should work");

    *offset = end;

    Ok(u16::from_le_bytes(buf))
}

pub fn parse_le_u32(offset: &mut usize, data: &[u8]) -> Result<u32> {
    let end = offset.checked_add(4).ok_or(ApfsError::InputTooSmall)?;
    let buf: [u8; 4] = data
        .get(*offset..end)
        .ok_or(ApfsError::InputTooSmall)?
        .try_into()
        .expect("buffer coercion should work");

    *offset = end;

    Ok(u32::from_le_bytes(buf))
}

pub fn parse_le_i64(offset: &mut usize, data: &[u8]) -> Result<i64> {
    let end = offset.checked_add(8).ok_or(ApfsError::InputTooSmall)?;
    let buf: [u8; 8] = data
        .get(*offset..end)
        .ok_or(ApfsError::InputTooSmall)?
        .try_into()
        .expect("buffer coercion should work");

    *offset = end;

    Ok(i64::from_le_bytes(buf))
}

pub fn parse_le_u64(offset: &mut usize, data: &[u8]) -> Result<u64> {
    let end = offset.checked_add(8).ok_or(ApfsError::InputTooSmall)?;
    let buf: [u8; 8] = data
        .get(*offset..end)
        .ok_or(ApfsError::InputTooSmall)?
        .try_into()
        .expect("buffer coercion should work");

    *offset = end;

    Ok(u64::from_le_bytes(buf))
}
