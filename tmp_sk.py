from pathlib import Path

prg = Path(r"c:\Users\rober\Code\rust\kisel\roms") / "Solomon's Key (USA).nes"
prg = prg.read_bytes()[16 : 16 + 32768]

OP = {
    0x00: ("BRK", 1), 0x05: ("ORA zp", 2), 0x06: ("ASL zp", 2), 0x08: ("PHP", 1),
    0x09: ("ORA #", 2), 0x0A: ("ASL A", 1), 0x10: ("BPL", 2), 0x18: ("CLC", 1),
    0x20: ("JSR", 3), 0x24: ("BIT zp", 2), 0x25: ("AND zp", 2), 0x26: ("ROL zp", 2),
    0x28: ("PLP", 1), 0x29: ("AND #", 2), 0x2A: ("ROL A", 1), 0x2C: ("BIT abs", 3),
    0x30: ("BMI", 2), 0x38: ("SEC", 1), 0x40: ("RTI", 1), 0x45: ("EOR zp", 2),
    0x48: ("PHA", 1), 0x49: ("EOR #", 2), 0x4A: ("LSR A", 1), 0x4C: ("JMP", 3),
    0x50: ("BVC", 2), 0x60: ("RTS", 1), 0x65: ("ADC zp", 2), 0x68: ("PLA", 1),
    0x69: ("ADC #", 2), 0x6A: ("ROR A", 1), 0x6C: ("JMP ind", 3), 0x70: ("BVS", 2),
    0x75: ("ADC zp,X", 2), 0x78: ("SEI", 1), 0x81: ("STA (zp,X)", 2), 0x84: ("STY zp", 2),
    0x85: ("STA zp", 2), 0x86: ("STX zp", 2), 0x88: ("DEY", 1), 0x8A: ("TXA", 1),
    0x8C: ("STY abs", 3), 0x8D: ("STA abs", 3), 0x8E: ("STX abs", 3), 0x90: ("BCC", 2),
    0x91: ("STA (zp),Y", 2), 0x95: ("STA zp,X", 2), 0x98: ("TYA", 1), 0x99: ("STA abs,Y", 3),
    0x9A: ("TXS", 1), 0x9D: ("STA abs,X", 3), 0xA0: ("LDY #", 2), 0xA1: ("LDA (zp,X)", 2),
    0xA2: ("LDX #", 2), 0xA4: ("LDY zp", 2), 0xA5: ("LDA zp", 2), 0xA6: ("LDX zp", 2),
    0xA8: ("TAY", 1), 0xA9: ("LDA #", 2), 0xAA: ("TAX", 1), 0xAC: ("LDY abs", 3),
    0xAD: ("LDA abs", 3), 0xAE: ("LDX abs", 3), 0xB0: ("BCS", 2), 0xB1: ("LDA (zp),Y", 2),
    0xB5: ("LDA zp,X", 2), 0xB9: ("LDA abs,Y", 3), 0xBA: ("TSX", 1), 0xBD: ("LDA abs,X", 3),
    0xC0: ("CPY #", 2), 0xC4: ("CPY zp", 2), 0xC5: ("CMP zp", 2), 0xC6: ("DEC zp", 2),
    0xC8: ("INY", 1), 0xC9: ("CMP #", 2), 0xCA: ("DEX", 1), 0xCD: ("CMP abs", 3),
    0xD0: ("BNE", 2), 0xD8: ("CLD", 1), 0xDD: ("CMP abs,X", 3), 0xE0: ("CPX #", 2),
    0xE6: ("INC zp", 2), 0xE8: ("INX", 1), 0xE9: ("SBC #", 2), 0xEA: ("NOP", 1),
    0xF0: ("BEQ", 2), 0xF6: ("INC zp,X", 2), 0xF8: ("SED", 1),
}


def dis(pc, n):
    end = pc + n
    while pc < end:
        op = prg[pc - 0x8000]
        name, size = OP.get(op, (f"??? {op:02X}", 1))
        raw = prg[pc - 0x8000 : pc - 0x8000 + size]
        arg = ""
        if size == 2:
            v = raw[1]
            if name.startswith("B") and "BIT" not in name and name != "BRK":
                tgt = pc + 2 + (v - 256 if v >= 128 else v)
                arg = f"${tgt:04X}"
            else:
                arg = f"${v:02X}"
        elif size == 3:
            addr = raw[1] + raw[2] * 256
            arg = f"${addr:04X}"
        print(f"  {pc:04X}  {raw.hex():<10} {name} {arg}")
        if name in ("RTS", "RTI", "JMP"):
            print("  ---")
            if name == "RTS":
                return
        pc += size


print("=== 8DB4 ===")
dis(0x8DB4, 80)
print("=== 8D5F ===")
dis(0x8D5F, 80)
