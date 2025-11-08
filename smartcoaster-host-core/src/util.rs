// Copyright (C) 2025 Paul Hampson
//
// This program is free software: you can redistribute it and/or modify it under
// the terms of the GNU General Public License version 3 as  published by the
// Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE.  See the GNU General Public License for more
// details.
//
// You should have received a copy of the GNU General Public License along with
// this program.  If not, see <https://www.gnu.org/licenses/>.

/// Calculates the Ascon-Hash256 of the given data
pub(crate) fn calculate_ascon_hash256(data: &[u8]) -> [u8; 32] {
    use ascon_hash::digest::Digest;
    use ascon_hash::AsconHash256;

    let mut hasher = AsconHash256::new();
    hasher.update(data);
    let result = hasher.finalize();

    let mut hash_bytes = [0u8; 32];
    hash_bytes.copy_from_slice(&result[..32]);
    hash_bytes
}