/*
   Copyright (C) 2026 Kamila Szewczyk <k@iczelia.net> on the behalf of the Saarland University.

   This program is free software; you can redistribute it and/or modify
   it under the terms of the GNU General Public License as published by
   the Free Software Foundation; either version 3 of the License, or
   (at your option) any later version.

   This program is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
   GNU General Public License for more details.

   You should have received a copy of the GNU General Public License
   along with this program. If not, see <http://www.gnu.org/licenses/>.
*/

use super::aux_model::AuxModel;
use super::model::MarkovModel;

pub const PRIMARY_MODEL_COUNT: usize = 5;

pub const MODEL_ID_O3: usize = 0;
pub const MODEL_ID_O7: usize = 1;
pub const MODEL_ID_O11: usize = 2;
pub const MODEL_ID_O15: usize = 3;
pub const MODEL_ID_O13: usize = 4;

pub type ModelO3 = MarkovModel<3, 0, false, false, 65536>;
pub type ModelO7 = MarkovModel<7, 0, false, false, 1023>;
pub type ModelO11 = MarkovModel<11, 2, false, true, 255>;
pub type ModelO15 = MarkovModel<15, 6, true, true, 15>;
pub type ModelO13 = MarkovModel<13, 9, true, false, 0>;

#[derive(Clone, Copy, Debug)]
pub struct PrimaryModelDescriptor {
    pub id: usize,
    pub name: &'static str,
    pub order: usize,
    pub alpha_den: u32,
    pub update_nonbest_counts: bool,
    pub update_reverse_complement: bool,
    pub configured_max_count: u32,
    pub effective_max_count: u32,
    pub storage_mode: u8,
    pub alloc_size_bytes: usize,
}

pub const PRIMARY_MODEL_DESCRIPTORS: [PrimaryModelDescriptor; PRIMARY_MODEL_COUNT] = [
    PrimaryModelDescriptor {
        id: MODEL_ID_O3,
        name: "o3",
        order: ModelO3::order(),
        alpha_den: ModelO3::alpha_den(),
        update_nonbest_counts: ModelO3::update_reg_enabled(),
        update_reverse_complement: ModelO3::update_rc_enabled(),
        configured_max_count: ModelO3::configured_max_count(),
        effective_max_count: ModelO3::effective_max_count(),
        storage_mode: ModelO3::storage_mode(),
        alloc_size_bytes: ModelO3::alloc_size_bytes(),
    },
    PrimaryModelDescriptor {
        id: MODEL_ID_O7,
        name: "o7",
        order: ModelO7::order(),
        alpha_den: ModelO7::alpha_den(),
        update_nonbest_counts: ModelO7::update_reg_enabled(),
        update_reverse_complement: ModelO7::update_rc_enabled(),
        configured_max_count: ModelO7::configured_max_count(),
        effective_max_count: ModelO7::effective_max_count(),
        storage_mode: ModelO7::storage_mode(),
        alloc_size_bytes: ModelO7::alloc_size_bytes(),
    },
    PrimaryModelDescriptor {
        id: MODEL_ID_O11,
        name: "o11",
        order: ModelO11::order(),
        alpha_den: ModelO11::alpha_den(),
        update_nonbest_counts: ModelO11::update_reg_enabled(),
        update_reverse_complement: ModelO11::update_rc_enabled(),
        configured_max_count: ModelO11::configured_max_count(),
        effective_max_count: ModelO11::effective_max_count(),
        storage_mode: ModelO11::storage_mode(),
        alloc_size_bytes: ModelO11::alloc_size_bytes(),
    },
    PrimaryModelDescriptor {
        id: MODEL_ID_O15,
        name: "o15",
        order: ModelO15::order(),
        alpha_den: ModelO15::alpha_den(),
        update_nonbest_counts: ModelO15::update_reg_enabled(),
        update_reverse_complement: ModelO15::update_rc_enabled(),
        configured_max_count: ModelO15::configured_max_count(),
        effective_max_count: ModelO15::effective_max_count(),
        storage_mode: ModelO15::storage_mode(),
        alloc_size_bytes: ModelO15::alloc_size_bytes(),
    },
    PrimaryModelDescriptor {
        id: MODEL_ID_O13,
        name: "o13",
        order: ModelO13::order(),
        alpha_den: ModelO13::alpha_den(),
        update_nonbest_counts: ModelO13::update_reg_enabled(),
        update_reverse_complement: ModelO13::update_rc_enabled(),
        configured_max_count: ModelO13::configured_max_count(),
        effective_max_count: ModelO13::effective_max_count(),
        storage_mode: ModelO13::storage_mode(),
        alloc_size_bytes: ModelO13::alloc_size_bytes(),
    },
];

pub const PRIMARY_MODELS_MEMORY_BYTES: usize = ModelO3::alloc_size_bytes()
    + ModelO7::alloc_size_bytes()
    + ModelO11::alloc_size_bytes()
    + ModelO15::alloc_size_bytes()
    + ModelO13::alloc_size_bytes();

pub const AUX_MODELS_MEMORY_BYTES: usize = AuxModel::<PRIMARY_MODEL_COUNT>::alloc_size_bytes()
    + AuxModel::<2>::alloc_size_bytes()
    + AuxModel::<2>::alloc_size_bytes()
    + AuxModel::<12>::alloc_size_bytes();

pub const MARKOV_MIX_MEMORY_BYTES: usize = PRIMARY_MODELS_MEMORY_BYTES + AUX_MODELS_MEMORY_BYTES;
