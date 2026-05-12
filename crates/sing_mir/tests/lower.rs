use sing_mir::{lower_source, MirOp, MirTerminator, Rvalue};

fn function(src: &str, name: &str) -> sing_mir::MirFunction {
    let program = lower_source(src).expect("source should lower");
    program
        .functions
        .into_iter()
        .find(|func| func.name == name)
        .expect("expected function")
}

#[test]
fn lowering_desugars_binds_and_folds_constants() {
    let func = function("f main()>i4{x=1+2;x}", "main");

    assert_eq!(func.blocks.len(), 1);
    assert!(matches!(
        func.blocks[0].ops.first(),
        Some(MirOp::Assign {
            target,
            value: Rvalue::ConstInt(3)
        }) if target == "x"
    ));
    assert!(matches!(
        &func.blocks[0].term,
        MirTerminator::Return(Some(Rvalue::Use(name))) if name == "x"
    ));
    assert_eq!(func.optimizations[0].name, "const-fold");
    assert_eq!(func.dataflow.locals["x"].defs, 1);
    assert_eq!(func.dataflow.locals["x"].uses, 1);
}

#[test]
fn ternary_lowers_to_explicit_branch_blocks() {
    let func = function("f pick()>i4:1?2:3", "pick");

    assert_eq!(func.blocks.len(), 3);
    assert!(matches!(
        func.blocks[0].term,
        MirTerminator::Branch {
            then_block: 1,
            else_block: 2,
            ..
        }
    ));
    assert!(func.blocks.iter().all(|block| block.reachable));
}

#[test]
fn return_creates_unreachable_dead_block_summary() {
    let func = function("f main()>i4{>1;2}", "main");

    assert_eq!(func.blocks.len(), 2);
    assert!(matches!(
        func.blocks[0].term,
        MirTerminator::Return(Some(Rvalue::ConstInt(1)))
    ));
    assert!(!func.blocks[1].reachable);
    assert_eq!(func.dead_blocks, vec![1]);
    assert_eq!(func.optimizations[0].name, "dead-block");
}

#[test]
fn dataflow_tracks_defs_and_uses() {
    let func = function("f main()>i4{x=1;y=x+2;y}", "main");

    assert_eq!(func.dataflow.locals["x"].defs, 1);
    assert_eq!(func.dataflow.locals["x"].uses, 1);
    assert_eq!(func.dataflow.locals["y"].defs, 1);
    assert_eq!(func.dataflow.locals["y"].uses, 1);
}
