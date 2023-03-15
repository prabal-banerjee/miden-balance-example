use miden::{Assembler, ProofOptions, ProgramInputs, AdviceSet};
use miden_core::{Word, Felt, FieldElement, StarkField};

fn main() {
    let source = format!(
            "  
            begin
                # stack = [alice_index, bob_index, transfer_value, depth, ROOT, ...]
                dupw.1 dup.4 dup.8
                # stack = [depth, alice_index, ROOT, alice_index, bob_index, transfer_value, depth, ROOT, ...]
                mtree_get
                # stack = [ALICE_BALANCE, ROOT, alice_index, bob_index, transfer_value, depth, ROOT, ...]

                drop drop dup.8
                # stack = [transfer_value, 0, alice_balance, ROOT, alice_index, bob_index, transfer_value, depth, ROOT, ...]
                # assert transfer_value > 0
                lt assert
                # stack = [alice_balance, ROOT, alice_index, bob_index, transfer_value, depth, ROOT, ...]
                # assert alice_balance >= transfer_value
                dup dup.8 gte assert
                # stack = [alice_balance, ROOT, alice_index, bob_index, transfer_value, depth, ROOT, ...]

                dup.7 sub
                # stack = [new_alice_balance, ROOT, alice_index, bob_index, transfer_value, depth, ROOT, ...]
                push.0.0.0 swapw movup.8 dup.11
                # stack = [depth, alice_index, ROOT, NEW_ALICE_BALANCE, bob_index, transfer_value, depth, ROOT, ...]
                mtree_set
                # stack = [NEW_ROOT, NEW_ALICE_BALANCE, bob_index, transfer_value, depth, ROOT, ...]

                # TODO: Verify bob's leaf existence before updating alice's leaf
                dup.8 dup.11
                # stack = [depth, bob_index, NEW_ROOT, NEW_ALICE_BALANCE, bob_index, transfer_value, depth, ...]
                mtree_get
                # stack = [BOB_BALANCE, NEW_ROOT, NEW_ALICE_BALANCE, bob_index, transfer_value, depth, ...]
                movup.3 movup.13 add movdn.3
                # stack = [NEW_BOB_BALANCE, NEW_ROOT, NEW_ALICE_BALANCE, bob_index, depth, ...]
                swapw movup.12 movup.13
                # stack = [depth, bob_index, NEW_ROOT, NEW_BOB_BALANCE, NEW_ALICE_BALANCE, ...]
                mtree_set

            end
            ",
        );

        // Compiling the program
        let assembler = Assembler::default();
        let program = assembler
            .compile(source.as_str())
            .expect("Could not compile source");

        // Program to verify that leaf of a Merkle tree is correct
        // Let the leaves fixed for now
        let mut leaves: Vec<u64> = vec![20, 20, 20, 20];
        let alice_index: usize = 2;     // sender
        let bob_index: usize = 3;       // receiver
        let transfer_value: u64 = 5;    // amount

        // Construct the Merkle tree leaves. Every leaf is a word. 
        let mut mtree_leaves: Vec<Word> = Vec::new();
        for i in 0..leaves.len() {
            mtree_leaves.push([
                Felt::new(leaves[i]),
                Felt::ZERO,
                Felt::ZERO,
                Felt::ZERO,
            ]);
        }

        let mtree = AdviceSet::new_merkle_tree(mtree_leaves).unwrap();
        let mtree_root = mtree.root().iter().map(|x| x.as_int()).collect::<Vec<u64>>();

        // stack = [alice_index, bob_index, transfer_value, depth, ROOT]
        let mut stack_init = Vec::<u64>::new();
        stack_init.push(alice_index.try_into().unwrap());
        stack_init.push(bob_index.try_into().unwrap());
        stack_init.push(transfer_value);
        stack_init.push(mtree.depth().into());
        for i in mtree_root.iter().rev() {
            stack_init.push(*i)
        }
        stack_init.reverse();

        let program_inputs = ProgramInputs::new(&stack_init, &[], vec![mtree.clone()]).unwrap();

        // Run the prover. Generate the proof. 
        let (output, proof) =
            miden::prove(
                &program, 
                &program_inputs, 
                &ProofOptions::with_96_bit_security()
            )
            .expect("results");

        // Print the output
        println!("Output of the program: {:?}", output);
        
        // Construct the modified Merkle tree. Assert that computation inside VM is correct
        // Assume that VM execution worked, i.e., assertions passed
        leaves[alice_index] = leaves[alice_index] - transfer_value;
        leaves[bob_index] = leaves[bob_index] + transfer_value;

        let mut modified_mtree_leaves: Vec<Word> = Vec::new();
        for i in 0..leaves.len() {
            modified_mtree_leaves.push([
                Felt::new(leaves[i]),
                Felt::ZERO,
                Felt::ZERO,
                Felt::ZERO,
            ]);
        }

        let modified_mtree = AdviceSet::new_merkle_tree(modified_mtree_leaves).unwrap();
        let modified_mtree_root = modified_mtree.root().iter().map(|x| x.as_int()).collect::<Vec<u64>>();

        for i in 0..4 {
            assert_eq!(modified_mtree_root[i], output.stack()[3-i], "Root update incorrect!");
        }
        
        let program_input_u64 = program_inputs
            .stack_init()
            .iter()
            .map(|x| x.as_int())
            .rev()
            .collect::<Vec<u64>>();

        // Verify the proof generated above
        match miden::verify(
            program.hash(),
            &program_input_u64,
            &output,
            proof,
        ) {
            Ok(_) => println!("Execution verified!"),
            Err(msg) => println!("Something went terribly wrong: {}", msg),
        }

}