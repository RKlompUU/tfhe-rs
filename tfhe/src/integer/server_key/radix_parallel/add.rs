use crate::integer::ciphertext::RadixCiphertext;
use crate::integer::ServerKey;
use crate::shortint::PBSOrderMarker;

use rayon::prelude::*;

/// Enum to tell parallel additions whether they should
/// add an extra one to the first pair of block being added
///
/// It is useful when implementing the parallel subtraction
pub(crate) enum AddExtraOne {
    Yes,
    No,
}

#[repr(u64)]
#[derive(PartialEq, Eq)]
enum OutputCarry {
    /// The block does not generate nor propagate a carry
    None = 0,
    /// The block generates a carry
    Generated = 1,
    /// The block will propagate a carry if it ever
    /// receives one
    Propagated = 2,
}

/// Function to create the LUT used in parallel prefix sum
/// to compute carry propagation
///
/// If msb propagates it take the value of lsb,
/// this means:
/// - if lsb propagates, msb will propagate (but we don't know yet if there will actually be a carry
///   to propagate),
/// - if lsb generates a carry, as msb propagates it, lsb will generate a carry. Note that this lsb
///   generates might be due to x propagating ('resolved' by an earlier iteration of the loop)
/// - if lsb does not ouput a carry, msb will have nothing to propagate
///
/// Otherwise, msb either does not generate, or it does generate,
/// but it means it won't propagate
fn prefix_sum_carry_propagation(msb: u64, lsb: u64) -> u64 {
    if msb == OutputCarry::Propagated as u64 {
        lsb
    } else {
        msb
    }
}

impl ServerKey {
    /// Computes homomorphically an addition between two ciphertexts encrypting integer values.
    ///
    /// # Warning
    ///
    /// - Multithreaded
    ///
    /// # Example
    ///
    /// ```rust
    /// use tfhe::integer::gen_keys_radix;
    /// use tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2;
    ///
    /// // Generate the client key and the server key:
    /// let num_blocks = 4;
    /// let (cks, sks) = gen_keys_radix(PARAM_MESSAGE_2_CARRY_2, num_blocks);
    ///
    /// let msg1 = 14;
    /// let msg2 = 97;
    ///
    /// let mut ct1 = cks.encrypt(msg1);
    /// let mut ct2 = cks.encrypt(msg2);
    ///
    /// // Compute homomorphically an addition:
    /// let ct_res = sks.smart_add_parallelized(&mut ct1, &mut ct2);
    ///
    /// // Decrypt:
    /// let dec_result: u64 = cks.decrypt(&ct_res);
    /// assert_eq!(dec_result, msg1 + msg2);
    /// ```
    pub fn smart_add_parallelized<PBSOrder: PBSOrderMarker>(
        &self,
        ct_left: &mut RadixCiphertext<PBSOrder>,
        ct_right: &mut RadixCiphertext<PBSOrder>,
    ) -> RadixCiphertext<PBSOrder> {
        if !self.is_add_possible(ct_left, ct_right) {
            rayon::join(
                || self.full_propagate_parallelized(ct_left),
                || self.full_propagate_parallelized(ct_right),
            );
        }
        self.unchecked_add(ct_left, ct_right)
    }

    pub fn smart_add_assign_parallelized<PBSOrder: PBSOrderMarker>(
        &self,
        ct_left: &mut RadixCiphertext<PBSOrder>,
        ct_right: &mut RadixCiphertext<PBSOrder>,
    ) {
        if !self.is_add_possible(ct_left, ct_right) {
            rayon::join(
                || self.full_propagate_parallelized(ct_left),
                || self.full_propagate_parallelized(ct_right),
            );
        }
        self.unchecked_add_assign(ct_left, ct_right);
    }

    /// Computes homomorphically an addition between two ciphertexts encrypting integer values.
    ///
    /// This function, like all "default" operations (i.e. not smart, checked or unchecked), will
    /// check that the input ciphertexts block carries are empty and clears them if it's not the
    /// case and the operation requires it. It outputs a ciphertext whose block carries are always
    /// empty.
    ///
    /// This means that when using only "default" operations, a given operation (like add for
    /// example) has always the same performance characteristics from one call to another and
    /// guarantees correctness by pre-emptively clearing carries of output ciphertexts.
    ///
    /// # Warning
    ///
    /// - Multithreaded
    ///
    /// # Example
    ///
    /// ```rust
    /// use tfhe::integer::gen_keys_radix;
    /// use tfhe::shortint::parameters::PARAM_MESSAGE_2_CARRY_2;
    ///
    /// // Generate the client key and the server key:
    /// let num_blocks = 4;
    /// let (cks, sks) = gen_keys_radix(PARAM_MESSAGE_2_CARRY_2, num_blocks);
    ///
    /// let msg1 = 14;
    /// let msg2 = 97;
    ///
    /// let ct1 = cks.encrypt(msg1);
    /// let ct2 = cks.encrypt(msg2);
    ///
    /// // Compute homomorphically an addition:
    /// let ct_res = sks.add_parallelized(&ct1, &ct2);
    ///
    /// // Decrypt:
    /// let dec_result: u64 = cks.decrypt(&ct_res);
    /// assert_eq!(dec_result, msg1 + msg2);
    /// ```
    pub fn add_parallelized<PBSOrder: PBSOrderMarker>(
        &self,
        ct_left: &RadixCiphertext<PBSOrder>,
        ct_right: &RadixCiphertext<PBSOrder>,
    ) -> RadixCiphertext<PBSOrder> {
        let mut ct_res = ct_left.clone();
        self.add_assign_parallelized(&mut ct_res, ct_right);
        ct_res
    }

    pub fn add_assign_parallelized<PBSOrder: PBSOrderMarker>(
        &self,
        ct_left: &mut RadixCiphertext<PBSOrder>,
        ct_right: &RadixCiphertext<PBSOrder>,
    ) {
        let mut tmp_rhs: RadixCiphertext<PBSOrder>;

        let (lhs, rhs) = match (
            ct_left.block_carries_are_empty(),
            ct_right.block_carries_are_empty(),
        ) {
            (true, true) => (ct_left, ct_right),
            (true, false) => {
                tmp_rhs = ct_right.clone();
                self.full_propagate_parallelized(&mut tmp_rhs);
                (ct_left, &tmp_rhs)
            }
            (false, true) => {
                self.full_propagate_parallelized(ct_left);
                (ct_left, ct_right)
            }
            (false, false) => {
                tmp_rhs = ct_right.clone();
                rayon::join(
                    || self.full_propagate_parallelized(ct_left),
                    || self.full_propagate_parallelized(&mut tmp_rhs),
                );
                (ct_left, &tmp_rhs)
            }
        };

        if self.is_eligible_for_parallel_carryless_add() {
            self.unchecked_add_assign_parallelized_low_latency(lhs, rhs, AddExtraOne::No);
        } else {
            self.unchecked_add_assign(lhs, rhs);
            self.full_propagate_parallelized(lhs);
        }
    }

    pub fn add_parallelized_work_efficient<PBSOrder: PBSOrderMarker>(
        &self,
        ct_left: &RadixCiphertext<PBSOrder>,
        ct_right: &RadixCiphertext<PBSOrder>,
    ) -> RadixCiphertext<PBSOrder> {
        let mut ct_res = ct_left.clone();
        self.add_assign_parallelized_work_efficient(&mut ct_res, ct_right);
        ct_res
    }

    pub fn add_assign_parallelized_work_efficient<PBSOrder: PBSOrderMarker>(
        &self,
        ct_left: &mut RadixCiphertext<PBSOrder>,
        ct_right: &RadixCiphertext<PBSOrder>,
    ) {
        let mut tmp_rhs: RadixCiphertext<PBSOrder>;

        let (lhs, rhs) = match (
            ct_left.block_carries_are_empty(),
            ct_right.block_carries_are_empty(),
        ) {
            (true, true) => (ct_left, ct_right),
            (true, false) => {
                tmp_rhs = ct_right.clone();
                self.full_propagate_parallelized(&mut tmp_rhs);
                (ct_left, &tmp_rhs)
            }
            (false, true) => {
                self.full_propagate_parallelized(ct_left);
                (ct_left, ct_right)
            }
            (false, false) => {
                tmp_rhs = ct_right.clone();
                rayon::join(
                    || self.full_propagate_parallelized(ct_left),
                    || self.full_propagate_parallelized(&mut tmp_rhs),
                );
                (ct_left, &tmp_rhs)
            }
        };

        self.unchecked_add_assign_parallelized_work_efficient(lhs, rhs, AddExtraOne::No);
    }

    pub(crate) fn is_eligible_for_parallel_carryless_add(&self) -> bool {
        // having 4-bits is a hard requirement
        // as the parallel implementation uses a bivariate BPS where individual values need
        // 2 bits
        let total_modulus = self.key.message_modulus.0 * self.key.carry_modulus.0;
        total_modulus >= (1 << 4)
    }

    /// This add_assign two numbers
    ///
    /// It uses the Hillis and Steele algorithm to do
    /// prefix sum / cumulative sum in parallel.
    ///
    /// It it not "work efficient" as in, it adds a lot
    /// of work compared to the single threaded aproach,
    /// however it is higly parallelized and so is the fastest
    /// assuming enough threads are available.
    ///
    /// At most num_block - 1 threads are used
    ///
    /// # Requirements
    ///
    /// - The parameters have 4 bits in total
    /// - The input carries of both lhs and rhs must be empty
    ///
    /// # Output
    ///
    /// - lhs will have its carries empty
    pub(crate) fn unchecked_add_assign_parallelized_low_latency<PBSOrder: PBSOrderMarker>(
        &self,
        lhs: &mut RadixCiphertext<PBSOrder>,
        rhs: &RadixCiphertext<PBSOrder>,
        add_extra_one: AddExtraOne,
    ) {
        debug_assert!(lhs.block_carries_are_empty());
        debug_assert!(rhs.block_carries_are_empty());
        debug_assert!(self.key.message_modulus.0 * self.key.carry_modulus.0 >= (1 << 3));

        let mut carry_out = self.add_and_generate_init_carry_array(lhs, rhs, add_extra_one);

        let num_blocks = carry_out.len();
        let num_steps = carry_out.len().ilog2() as usize;

        let lut_carry_propagation_sum = self
            .key
            .generate_accumulator_bivariate(prefix_sum_carry_propagation);

        let mut space = 1;
        let mut step_output = carry_out.clone();
        for _ in 0..=num_steps {
            step_output[space..num_blocks]
                .par_iter_mut()
                .enumerate()
                .for_each(|(i, block)| {
                    let prev_block_carry = &carry_out[i];
                    self.key.unchecked_apply_lookup_table_bivariate_assign(
                        block,
                        prev_block_carry,
                        &lut_carry_propagation_sum,
                    )
                });
            for i in space..num_blocks {
                carry_out[i].copy_from(&step_output[i]);
            }

            space *= 2;
        }

        // The output carry of block i-1 becomes the input
        // carry of block i
        carry_out.rotate_right(1);
        self.key.create_trivial_assign(&mut carry_out[0], 0);
        lhs.blocks
            .par_iter_mut()
            .zip(carry_out.par_iter())
            .for_each(|(block, input_carry)| {
                self.key.unchecked_add_assign(block, input_carry);
                self.key.message_extract_assign(block);
            });
    }

    /// This add_assign two numbers
    ///
    /// It is after the Blelloch algorithm to do
    /// prefix sum / cumulative sum in parallel.
    ///
    /// It is not "work efficient" as in, it does not adds
    /// that much work compared to other parallel algorithm,
    /// thus requiring less threads.
    ///
    /// However it is slower.
    ///
    /// At most num_block / 2 threads are used
    ///
    /// # Requirements
    ///
    /// - The parameters have 4 bits in total
    /// - The input carries of both lhs and rhs must be empty
    ///
    /// # Output
    ///
    /// - lhs will have its carries empty
    pub(crate) fn unchecked_add_assign_parallelized_work_efficient<PBSOrder: PBSOrderMarker>(
        &self,
        lhs: &mut RadixCiphertext<PBSOrder>,
        rhs: &RadixCiphertext<PBSOrder>,
        add_extra_one: AddExtraOne,
    ) {
        debug_assert!(lhs.block_carries_are_empty());
        debug_assert!(rhs.block_carries_are_empty());
        debug_assert!(self.key.message_modulus.0 * self.key.carry_modulus.0 >= (1 << 3));

        let mut carry_out = self.add_and_generate_init_carry_array(lhs, rhs, add_extra_one);

        let num_blocks = carry_out.len();
        let num_steps = carry_out.len().ilog2() as usize;

        let lut_carry_propagation_sum = self
            .key
            .generate_accumulator_bivariate(prefix_sum_carry_propagation);

        use std::cell::UnsafeCell;

        #[derive(Copy, Clone)]
        pub struct UnsafeSlice<'a, T> {
            slice: &'a [UnsafeCell<T>],
        }
        unsafe impl<'a, T: Send + Sync> Send for UnsafeSlice<'a, T> {}
        unsafe impl<'a, T: Send + Sync> Sync for UnsafeSlice<'a, T> {}

        impl<'a, T> UnsafeSlice<'a, T> {
            pub fn new(slice: &'a mut [T]) -> Self {
                let ptr = slice as *mut [T] as *const [UnsafeCell<T>];
                Self {
                    slice: unsafe { &*ptr },
                }
            }

            /// SAFETY: It is UB if two threads read/write the pointer without synchronisation
            pub unsafe fn get(&self, i: usize) -> *mut T {
                self.slice[i].get()
            }
        }

        let carry_out_slice = UnsafeSlice::new(&mut carry_out);
        for i in 0..num_steps {
            let two_pow_i_plus_1 = 2usize.checked_pow((i + 1) as u32).unwrap();
            let two_pow_i = 2usize.checked_pow(i as u32).unwrap();

            (0..num_blocks)
                .into_par_iter()
                .step_by(two_pow_i_plus_1)
                .for_each(|k| {
                    let current_index = k + two_pow_i_plus_1 - 1;
                    let previous_index = k + two_pow_i - 1;

                    unsafe {
                        // SAFETY
                        // We know none of the threads
                        // are going to access the same pointers
                        let current_block = carry_out_slice.get(current_index);
                        let previous_block = carry_out_slice.get(previous_index) as *const _;

                        self.key.unchecked_apply_lookup_table_bivariate_assign(
                            &mut *current_block,
                            &*previous_block,
                            &lut_carry_propagation_sum,
                        );
                    }
                });
        }

        // Down-Sweep phase
        let mut buffer = Vec::with_capacity(num_blocks / 2);
        self.key
            .create_trivial_assign(&mut carry_out[num_blocks - 1], 0);
        for i in (0..num_steps).rev() {
            let two_pow_i_plus_1 = 2usize.checked_pow((i + 1) as u32).unwrap();
            let two_pow_i = 2usize.checked_pow(i as u32).unwrap();

            (0..num_blocks)
                .into_par_iter()
                .step_by(two_pow_i_plus_1)
                .map(|k| {
                    // Since our carry_propagation LUT ie sum function
                    // is not commutative we have to reverse operands
                    self.key.unchecked_apply_lookup_table_bivariate(
                        &carry_out[k + two_pow_i - 1],
                        &carry_out[k + two_pow_i_plus_1 - 1],
                        &lut_carry_propagation_sum,
                    )
                })
                .collect_into_vec(&mut buffer);

            let mut drainer = buffer.drain(..);
            for k in (0..num_blocks).step_by(two_pow_i_plus_1) {
                let b = drainer.next().unwrap();
                carry_out.swap(k + two_pow_i - 1, k + two_pow_i_plus_1 - 1);
                carry_out[k + two_pow_i_plus_1 - 1] = b;
            }
            drop(drainer);
            assert!(buffer.is_empty());
        }

        // The first step of the Down-Sweep phase sets the
        // first block to 0, so no need to re-do it
        lhs.blocks
            .par_iter_mut()
            .zip(carry_out.par_iter())
            .for_each(|(block, carry_in)| {
                self.key.unchecked_add_assign(block, carry_in);
                self.key.message_extract_assign(block);
            });
    }

    /// Initialization function for parallal carryless sum
    ///
    /// This function adds rhs into lhs
    ///
    /// It returns the array that tells whether the
    /// block resulting from the sum will propagate or generate
    /// a carry. A Prefix sum / cumulative sum then needs to be done
    /// to get the final carry that the block will output
    fn add_and_generate_init_carry_array<PBSOrder: PBSOrderMarker>(
        &self,
        lhs: &mut RadixCiphertext<PBSOrder>,
        rhs: &RadixCiphertext<PBSOrder>,
        add_extra_one: AddExtraOne,
    ) -> Vec<crate::shortint::CiphertextBase<PBSOrder>> {
        let modulus = self.key.message_modulus.0 as u64;

        // This used for the first pair of blocks
        // as this pair can either generate or not, but never propagate
        let lut_does_block_generate_carry = self.key.generate_accumulator(|x| {
            if x >= modulus {
                OutputCarry::Generated as u64
            } else {
                OutputCarry::None as u64
            }
        });

        let lut_does_block_generate_or_propagate = self.key.generate_accumulator(|x| {
            if x >= modulus {
                OutputCarry::Generated as u64
            } else if x == (modulus - 1) {
                OutputCarry::Propagated as u64
            } else {
                OutputCarry::None as u64
            }
        });

        let mut carry_out = Vec::with_capacity(lhs.blocks.len());
        lhs.blocks
            .par_iter_mut()
            .zip(rhs.blocks.par_iter())
            .enumerate()
            .map(|(i, (ct_left_i, ct_right_i))| {
                self.key.unchecked_add_assign(ct_left_i, ct_right_i);
                if i == 0 {
                    if matches!(add_extra_one, AddExtraOne::Yes) {
                        self.key.unchecked_scalar_add_assign(ct_left_i, 1);
                    }
                    // The first block can only ouput a carry
                    self.key
                        .apply_lookup_table(ct_left_i, &lut_does_block_generate_carry)
                } else {
                    self.key
                        .apply_lookup_table(ct_left_i, &lut_does_block_generate_or_propagate)
                }
            })
            .collect_into_vec(&mut carry_out);

        carry_out
    }

    /// op must be associative and commutative
    pub fn smart_binary_op_seq_parallelized<'this, 'item, PBSOrder: PBSOrderMarker + 'item>(
        &'this self,
        ct_seq: impl IntoIterator<Item = &'item mut RadixCiphertext<PBSOrder>>,
        op: impl for<'a> Fn(
                &'a ServerKey,
                &'a mut RadixCiphertext<PBSOrder>,
                &'a mut RadixCiphertext<PBSOrder>,
            ) -> RadixCiphertext<PBSOrder>
            + Sync,
    ) -> Option<RadixCiphertext<PBSOrder>> {
        enum CiphertextCow<'a, O: PBSOrderMarker> {
            Borrowed(&'a mut RadixCiphertext<O>),
            Owned(RadixCiphertext<O>),
        }
        impl<O: PBSOrderMarker> CiphertextCow<'_, O> {
            fn as_mut(&mut self) -> &mut RadixCiphertext<O> {
                match self {
                    CiphertextCow::Borrowed(b) => b,
                    CiphertextCow::Owned(o) => o,
                }
            }
        }

        let ct_seq = ct_seq
            .into_iter()
            .map(CiphertextCow::Borrowed)
            .collect::<Vec<_>>();
        let op = &op;

        // overhead of dynamic dispatch is negligible compared to multithreading, PBS, etc.
        // we defer all calls to a single implementation to avoid code bloat and long compile
        // times
        #[allow(clippy::type_complexity)]
        fn reduce_impl<PBSOrder: PBSOrderMarker>(
            sks: &ServerKey,
            mut ct_seq: Vec<CiphertextCow<PBSOrder>>,
            op: &(dyn for<'a> Fn(
                &'a ServerKey,
                &'a mut RadixCiphertext<PBSOrder>,
                &'a mut RadixCiphertext<PBSOrder>,
            ) -> RadixCiphertext<PBSOrder>
                  + Sync),
        ) -> Option<RadixCiphertext<PBSOrder>> {
            use rayon::prelude::*;

            if ct_seq.is_empty() {
                None
            } else {
                // we repeatedly divide the number of terms by two by iteratively reducing
                // consecutive terms in the array
                let num_blocks = ct_seq[0].as_mut().blocks.len();
                while ct_seq.len() > 1 {
                    let mut results =
                        vec![sks.create_trivial_radix(0u64, num_blocks); ct_seq.len() / 2];

                    // if the number of elements is odd, we skip the first element
                    let untouched_prefix = ct_seq.len() % 2;
                    let ct_seq_slice = &mut ct_seq[untouched_prefix..];

                    results
                        .par_iter_mut()
                        .zip(ct_seq_slice.par_chunks_exact_mut(2))
                        .for_each(|(ct_res, chunk)| {
                            let (first, second) = chunk.split_at_mut(1);
                            let first = first[0].as_mut();
                            let second = second[0].as_mut();
                            *ct_res = op(sks, first, second);
                        });

                    ct_seq.truncate(untouched_prefix);
                    ct_seq.extend(results.into_iter().map(CiphertextCow::Owned));
                }

                let sum = ct_seq.pop().unwrap();

                Some(match sum {
                    CiphertextCow::Borrowed(b) => b.clone(),
                    CiphertextCow::Owned(o) => o,
                })
            }
        }

        reduce_impl(self, ct_seq, op)
    }

    /// op must be associative and commutative
    pub fn default_binary_op_seq_parallelized<'this, 'item, PBSOrder: PBSOrderMarker + 'item>(
        &'this self,
        ct_seq: impl IntoIterator<Item = &'item RadixCiphertext<PBSOrder>>,
        op: impl for<'a> Fn(
                &'a ServerKey,
                &'a RadixCiphertext<PBSOrder>,
                &'a RadixCiphertext<PBSOrder>,
            ) -> RadixCiphertext<PBSOrder>
            + Sync,
    ) -> Option<RadixCiphertext<PBSOrder>> {
        enum CiphertextCow<'a, O: PBSOrderMarker> {
            Borrowed(&'a RadixCiphertext<O>),
            Owned(RadixCiphertext<O>),
        }
        impl<O: PBSOrderMarker> CiphertextCow<'_, O> {
            fn as_ref(&self) -> &RadixCiphertext<O> {
                match self {
                    CiphertextCow::Borrowed(b) => b,
                    CiphertextCow::Owned(o) => o,
                }
            }
        }

        let ct_seq = ct_seq
            .into_iter()
            .map(CiphertextCow::Borrowed)
            .collect::<Vec<_>>();
        let op = &op;

        // overhead of dynamic dispatch is negligible compared to multithreading, PBS, etc.
        // we defer all calls to a single implementation to avoid code bloat and long compile
        // times
        #[allow(clippy::type_complexity)]
        fn reduce_impl<PBSOrder: PBSOrderMarker>(
            sks: &ServerKey,
            mut ct_seq: Vec<CiphertextCow<PBSOrder>>,
            op: &(dyn for<'a> Fn(
                &'a ServerKey,
                &'a RadixCiphertext<PBSOrder>,
                &'a RadixCiphertext<PBSOrder>,
            ) -> RadixCiphertext<PBSOrder>
                  + Sync),
        ) -> Option<RadixCiphertext<PBSOrder>> {
            use rayon::prelude::*;

            if ct_seq.is_empty() {
                None
            } else {
                // we repeatedly divide the number of terms by two by iteratively reducing
                // consecutive terms in the array
                let num_blocks = ct_seq[0].as_ref().blocks.len();
                while ct_seq.len() > 1 {
                    let mut results =
                        vec![sks.create_trivial_radix(0u64, num_blocks); ct_seq.len() / 2];
                    // if the number of elements is odd, we skip the first element
                    let untouched_prefix = ct_seq.len() % 2;
                    let ct_seq_slice = &mut ct_seq[untouched_prefix..];

                    results
                        .par_iter_mut()
                        .zip(ct_seq_slice.par_chunks_exact(2))
                        .for_each(|(ct_res, chunk)| {
                            let first = chunk[0].as_ref();
                            let second = chunk[1].as_ref();
                            *ct_res = op(sks, first, second);
                        });

                    ct_seq.truncate(untouched_prefix);
                    ct_seq.extend(results.into_iter().map(CiphertextCow::Owned));
                }

                let sum = ct_seq.pop().unwrap();

                Some(match sum {
                    CiphertextCow::Borrowed(b) => b.clone(),
                    CiphertextCow::Owned(o) => o,
                })
            }
        }

        reduce_impl(self, ct_seq, op)
    }
}
