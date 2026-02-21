// 1-modifiers: Const(˙), Swap(˜), Cells(˘), Each(¨), Table(⌜), Undo(⁼), Fold(´), Insert(˝), Scan(`)
//
// Native modifier dispatch is implemented in rbqn-vm/src/modifiers.rs because modifiers
// need to call derive::c1/c2 to apply operand functions, which would create a circular
// dependency if placed here in rbqn-prim.
//
// The dispatch table entries for modifier indices (44-52) have c1: None, c2: None because
// modifiers are not called directly as functions. Instead they are applied via MD1C/MD2C
// opcodes which create Md1D derived values, and the actual logic runs when those derived
// values are called through the NativeMd1 dispatch path in derive.rs.
