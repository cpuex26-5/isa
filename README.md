# isa

## 1st ISA

- RV32IF をベースとして、必要なさそうなものを削除する。
- 主な差分：
    - I からは lb, lh, sb, sh を消す
    - F からは次のものを採用する: add, sub, mul, div, fsqrt, fmin, fmax, feq, flt, fle, flw, fsw, fcvt.w.s, fcvt.s.w, fmv.x.w, fmv.w.x

