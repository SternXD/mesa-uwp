/*
 * Copyright © 2022 Collabora, Ltd.
 * SPDX-License-Identifier: MIT
 */

#![allow(non_upper_case_globals)]

use crate::nak_ir::*;
use crate::nir::*;

use nak_bindings::*;

struct ShaderFromNir<'a> {
    nir: &'a nir_shader,
    func: Option<Function>,
    instrs: Vec<Instr>,
    fs_out_regs: Vec<Src>,
}

impl<'a> ShaderFromNir<'a> {
    fn new(nir: &'a nir_shader) -> Self {
        let mut fs_out_regs = Vec::new();
        if nir.info.stage() == MESA_SHADER_FRAGMENT {
            fs_out_regs
                .resize(nir.num_outputs.try_into().unwrap(), Src::new_zero());
        }

        Self {
            nir: nir,
            func: None,
            instrs: Vec::new(),
            fs_out_regs: fs_out_regs,
        }
    }

    pub fn alloc_ssa(&mut self, file: RegFile, comps: u8) -> SSAValue {
        self.func.as_mut().unwrap().alloc_ssa(file, comps)
    }

    fn get_ssa(&self, def: &nir_def) -> SSAValue {
        if def.bit_size == 1 {
            SSAValue::new(RegFile::Pred, def.index, def.num_components)
        } else {
            assert!(def.bit_size == 32 || def.bit_size == 64);
            let dwords = (def.bit_size / 32) * def.num_components;
            //Src::new_ssa(def.index, dwords, !def.divergent)
            SSAValue::new(RegFile::GPR, def.index, dwords)
        }
    }

    fn get_src(&self, src: &nir_src) -> Src {
        self.get_ssa(&src.as_def()).into()
    }

    fn get_dst(&self, dst: &nir_def) -> Dst {
        self.get_ssa(dst).into()
    }

    fn get_alu_src(&mut self, alu_src: &nir_alu_src) -> Src {
        if alu_src.src.num_components() == 1 {
            self.get_src(&alu_src.src)
        } else {
            assert!(alu_src.src.bit_size() == 32);
            let vec_src = self.get_src(&alu_src.src);
            let comp =
                self.alloc_ssa(vec_src.src_ref.as_ssa().unwrap().file(), 1);
            let mut dsts = Vec::new();
            for c in 0..alu_src.src.num_components() {
                if c == alu_src.swizzle[0] {
                    dsts.push(comp.into());
                } else {
                    dsts.push(Dst::None);
                }
            }
            self.instrs.push(Instr::new_split(&dsts, vec_src));
            comp.into()
        }
    }

    fn parse_alu(&mut self, alu: &nir_alu_instr) {
        let mut srcs = Vec::new();
        for alu_src in alu.srcs_as_slice() {
            srcs.push(self.get_alu_src(alu_src));
        }
        let srcs = srcs;

        let dst = self.get_dst(&alu.def);

        match alu.op {
            nir_op_b2f32 => {
                self.instrs.push(Instr::new(Op::Sel(OpSel {
                    dst: dst,
                    cond: srcs[0].not(),
                    srcs: [Src::new_zero(), Src::new_imm_u32(0x3f800000)],
                })));
            }
            nir_op_bcsel => {
                self.instrs
                    .push(Instr::new_sel(dst, srcs[0], srcs[1], srcs[2]));
            }
            nir_op_fabs => {
                self.instrs.push(Instr::new(Op::FMov(OpFMov {
                    dst: dst,
                    src: srcs[0].abs(),
                    saturate: false,
                })));
            }
            nir_op_fadd => {
                self.instrs.push(Instr::new_fadd(dst, srcs[0], srcs[1]));
            }
            nir_op_feq => {
                self.instrs.push(Instr::new_fsetp(
                    dst,
                    FloatCmpOp::OrdEq,
                    srcs[0],
                    srcs[1],
                ));
            }
            nir_op_fge => {
                self.instrs.push(Instr::new_fsetp(
                    dst,
                    FloatCmpOp::OrdGe,
                    srcs[0],
                    srcs[1],
                ));
            }
            nir_op_flt => {
                self.instrs.push(Instr::new_fsetp(
                    dst,
                    FloatCmpOp::OrdLt,
                    srcs[0],
                    srcs[1],
                ));
            }
            nir_op_fneu => {
                self.instrs.push(Instr::new_fsetp(
                    dst,
                    FloatCmpOp::UnordNe,
                    srcs[0],
                    srcs[1],
                ));
            }
            nir_op_fneg => {
                self.instrs.push(Instr::new(Op::FMov(OpFMov {
                    dst: dst,
                    src: srcs[0].neg(),
                    saturate: false,
                })));
            }
            nir_op_fsat => {
                self.instrs.push(Instr::new(Op::FMov(OpFMov {
                    dst: dst,
                    src: srcs[0],
                    saturate: true,
                })));
            }
            nir_op_fsign => {
                let lz = self.alloc_ssa(RegFile::GPR, 1);
                self.instrs.push(Instr::new_fset(
                    lz.into(),
                    FloatCmpOp::OrdLt,
                    srcs[0],
                    Src::new_zero(),
                ));

                let gz = self.alloc_ssa(RegFile::GPR, 1);
                self.instrs.push(Instr::new_fset(
                    gz.into(),
                    FloatCmpOp::OrdGt,
                    srcs[0],
                    Src::new_zero(),
                ));

                self.instrs.push(Instr::new_fadd(
                    dst,
                    gz.into(),
                    Src::from(lz).neg(),
                ));
            }
            nir_op_i2f32 => {
                self.instrs.push(Instr::new_i2f(dst, srcs[0]));
            }
            nir_op_iabs => {
                self.instrs.push(Instr::new(Op::IMov(OpIMov {
                    dst: dst,
                    src: srcs[0].abs(),
                })));
            }
            nir_op_iadd => {
                self.instrs.push(Instr::new_iadd(dst, srcs[0], srcs[1]));
            }
            nir_op_iand => {
                if alu.def.bit_size() == 1 {
                    self.instrs.push(Instr::new_plop3(
                        dst,
                        LogicOp::new_lut(&|x, y, _| x & y),
                        srcs[0],
                        srcs[1],
                        Src::new_zero(),
                    ));
                } else {
                    self.instrs.push(Instr::new_lop3(
                        dst,
                        LogicOp::new_lut(&|x, y, _| x & y),
                        srcs[0],
                        srcs[1],
                        Src::new_zero(),
                    ));
                }
            }
            nir_op_ieq => {
                self.instrs.push(Instr::new_isetp(
                    dst,
                    IntCmpType::I32,
                    IntCmpOp::Eq,
                    srcs[0],
                    srcs[1],
                ));
            }
            nir_op_ige => {
                self.instrs.push(Instr::new_isetp(
                    dst,
                    IntCmpType::I32,
                    IntCmpOp::Ge,
                    srcs[0],
                    srcs[1],
                ));
            }
            nir_op_ilt => {
                self.instrs.push(Instr::new_isetp(
                    dst,
                    IntCmpType::I32,
                    IntCmpOp::Lt,
                    srcs[0],
                    srcs[1],
                ));
            }
            nir_op_ine => {
                self.instrs.push(Instr::new_isetp(
                    dst,
                    IntCmpType::I32,
                    IntCmpOp::Ne,
                    srcs[0],
                    srcs[1],
                ));
            }
            nir_op_ineg => {
                self.instrs.push(Instr::new(Op::IMov(OpIMov {
                    dst: dst,
                    src: srcs[0].neg(),
                })));
            }
            nir_op_inot => {
                if alu.def.bit_size() == 1 {
                    self.instrs.push(Instr::new_plop3(
                        dst,
                        LogicOp::new_lut(&|x, _, _| !x),
                        srcs[0],
                        Src::new_zero(),
                        Src::new_zero(),
                    ));
                } else {
                    self.instrs.push(Instr::new_lop3(
                        dst,
                        LogicOp::new_lut(&|x, _, _| !x),
                        srcs[0],
                        Src::new_zero(),
                        Src::new_zero(),
                    ));
                }
            }
            nir_op_ior => {
                if alu.def.bit_size() == 1 {
                    self.instrs.push(Instr::new_plop3(
                        dst,
                        LogicOp::new_lut(&|x, y, _| x | y),
                        srcs[0],
                        srcs[1],
                        Src::new_zero(),
                    ));
                } else {
                    self.instrs.push(Instr::new_lop3(
                        dst,
                        LogicOp::new_lut(&|x, y, _| x | y),
                        srcs[0],
                        srcs[1],
                        Src::new_zero(),
                    ));
                }
            }
            nir_op_ishl => {
                self.instrs.push(Instr::new_shl(dst, srcs[0], srcs[1]));
            }
            nir_op_mov => {
                self.instrs.push(Instr::new_mov(dst, srcs[0]));
            }
            nir_op_pack_64_2x32_split => {
                self.instrs.push(Instr::new_vec(dst, &[srcs[0], srcs[1]]));
            }
            nir_op_uge => {
                self.instrs.push(Instr::new_isetp(
                    dst,
                    IntCmpType::U32,
                    IntCmpOp::Ge,
                    srcs[0],
                    srcs[1],
                ));
            }
            nir_op_ult => {
                self.instrs.push(Instr::new_isetp(
                    dst,
                    IntCmpType::U32,
                    IntCmpOp::Lt,
                    srcs[0],
                    srcs[1],
                ));
            }
            _ => panic!("Unsupported ALU instruction"),
        }
    }

    fn parse_jump(&mut self, jump: &nir_jump_instr) {
        match jump.type_ {
            _ => panic!("Unsupported jump instruction"),
        }
    }

    fn parse_tex(&mut self, _tex: &nir_tex_instr) {
        panic!("Texture instructions unimplemented");
    }

    fn parse_intrinsic(&mut self, intrin: &nir_intrinsic_instr) {
        let srcs = intrin.srcs_as_slice();
        match intrin.intrinsic {
            nir_intrinsic_load_global => {
                let size_B =
                    (intrin.def.bit_size() / 8) * intrin.def.num_components();
                assert!(u32::from(size_B) <= intrin.align());
                let access = MemAccess {
                    addr_type: MemAddrType::A64,
                    mem_type: MemType::from_size(size_B, false),
                    order: MemOrder::Strong,
                    scope: MemScope::System,
                };
                let addr = self.get_src(&srcs[0]);
                let dst = self.get_dst(&intrin.def);
                self.instrs.push(Instr::new_ld(dst, access, addr));
            }
            nir_intrinsic_load_input => {
                let addr = u16::try_from(intrin.base()).unwrap();
                let vtx = Src::new_zero();
                let offset = self.get_src(&srcs[0]);
                let dst = self.get_dst(&intrin.def);
                self.instrs.push(Instr::new_ald(dst, addr, vtx, offset));
            }
            nir_intrinsic_load_per_vertex_input => {
                let addr = u16::try_from(intrin.base()).unwrap();
                let vtx = self.get_src(&srcs[0]);
                let offset = self.get_src(&srcs[1]);
                let dst = self.get_dst(&intrin.def);
                self.instrs.push(Instr::new_ald(dst, addr, vtx, offset));
            }
            nir_intrinsic_load_sysval_nv => {
                let idx = u8::try_from(intrin.base()).unwrap();
                let dst = self.get_dst(&intrin.def);
                self.instrs.push(Instr::new_s2r(dst, idx));
            }
            nir_intrinsic_load_ubo => {
                let idx = srcs[0];
                let offset = srcs[1];
                let dst = self.get_dst(&intrin.def);
                let dwords =
                    (intrin.def.bit_size() / 32) * intrin.def.num_components();
                if let Some(imm_idx) = idx.as_uint() {
                    let imm_idx = u8::try_from(imm_idx).unwrap();
                    if let Some(imm_offset) = offset.as_uint() {
                        let imm_offset = u16::try_from(imm_offset).unwrap();
                        let mut srcs = Vec::new();
                        for i in 0..dwords {
                            srcs.push(Src::new_cbuf(
                                imm_idx,
                                imm_offset + u16::from(i) * 4,
                            ));
                        }
                        self.instrs.push(Instr::new_vec(dst, &srcs));
                    } else {
                        panic!("Indirect UBO offsets not yet supported");
                    }
                } else {
                    panic!("Indirect UBO indices not yet supported");
                }
            }
            nir_intrinsic_store_global => {
                let data = self.get_src(&srcs[0]);
                let size_B =
                    (srcs[0].bit_size() / 8) * srcs[0].num_components();
                assert!(u32::from(size_B) <= intrin.align());
                let access = MemAccess {
                    addr_type: MemAddrType::A64,
                    mem_type: MemType::from_size(size_B, false),
                    order: MemOrder::Strong,
                    scope: MemScope::System,
                };
                let addr = self.get_src(&srcs[1]);
                self.instrs.push(Instr::new_st(access, addr, data));
            }
            nir_intrinsic_store_output => {
                if self.nir.info.stage() == MESA_SHADER_FRAGMENT {
                    /* We assume these only ever happen in the last block.
                     * This is ensured by nir_lower_io_to_temporaries()
                     */
                    let data = self.get_src(&srcs[0]);
                    assert!(srcs[1].is_zero());
                    let base: u8 = intrin.base().try_into().unwrap();
                    let mut dsts = Vec::new();
                    for c in 0..intrin.num_components {
                        let tmp = self.alloc_ssa(RegFile::GPR, 1);
                        self.fs_out_regs[(base + c) as usize] = tmp.into();
                        dsts.push(Dst::from(tmp));
                    }
                    self.instrs.push(Instr::new_split(&dsts, data))
                } else {
                    let data = self.get_src(&srcs[0]);
                    let vtx = Src::new_zero();
                    let offset = self.get_src(&srcs[1]);
                    let addr: u16 = intrin.base().try_into().unwrap();
                    self.instrs.push(Instr::new_ast(addr, data, vtx, offset))
                }
            }
            _ => panic!("Unsupported intrinsic instruction"),
        }
    }

    fn parse_load_const(&mut self, load_const: &nir_load_const_instr) {
        let dst = self.get_dst(&load_const.def);
        let mut srcs = Vec::new();
        for c in 0..load_const.def.num_components {
            assert!(load_const.def.bit_size == 32);
            let imm_u32 = unsafe { load_const.values()[c as usize].u32_ };
            srcs.push(if imm_u32 == 0 {
                Src::new_zero()
            } else {
                Src::new_imm_u32(imm_u32)
            });
        }
        self.instrs.push(Instr::new_vec(dst, &srcs));
    }

    fn parse_undef(&mut self, _ssa_undef: &nir_undef_instr) {
        panic!("SSA undef not implemented yet");
    }

    fn parse_basic_block(&mut self, nb: &nir_block) -> BasicBlock {
        for ni in nb.iter_instr_list() {
            match ni.type_ {
                nir_instr_type_alu => self.parse_alu(ni.as_alu().unwrap()),
                nir_instr_type_jump => self.parse_jump(ni.as_jump().unwrap()),
                nir_instr_type_tex => self.parse_tex(ni.as_tex().unwrap()),
                nir_instr_type_intrinsic => {
                    self.parse_intrinsic(ni.as_intrinsic().unwrap())
                }
                nir_instr_type_load_const => {
                    self.parse_load_const(ni.as_load_const().unwrap())
                }
                nir_instr_type_undef => {
                    self.parse_undef(ni.as_undef().unwrap())
                }
                _ => panic!("Unsupported instruction type"),
            }
        }
        let mut b = BasicBlock::new(0 /* TODO: Block indices */);
        b.instrs.append(&mut self.instrs);
        b
    }

    pub fn parse_function_impl(&mut self, nfi: &nir_function_impl) -> Function {
        self.func = Some(Function::new(0, nfi.ssa_alloc));
        for node in nfi.iter_body() {
            /* TODO: Control-flow */
            let b = self.parse_basic_block(node.as_block().unwrap());
            self.func.as_mut().unwrap().blocks.push(b);
        }

        let end_block = self.func.as_mut().unwrap().blocks.last_mut().unwrap();

        if self.nir.info.stage() == MESA_SHADER_FRAGMENT
            && nfi.function().is_entrypoint
        {
            let fs_out_regs =
                std::mem::replace(&mut self.fs_out_regs, Vec::new());
            end_block.instrs.push(Instr::new_fs_out(&fs_out_regs));
        }

        end_block.instrs.push(Instr::new_exit());

        self.func.take().unwrap()
    }

    pub fn parse_shader(&mut self, sm: u8) -> Shader {
        let mut s = Shader::new(sm);
        for nf in self.nir.iter_functions() {
            if let Some(nfi) = nf.get_impl() {
                let f = self.parse_function_impl(nfi);
                s.functions.push(f);
            }
        }
        s
    }
}

pub fn nak_shader_from_nir(ns: &nir_shader, sm: u8) -> Shader {
    ShaderFromNir::new(ns).parse_shader(sm)
}