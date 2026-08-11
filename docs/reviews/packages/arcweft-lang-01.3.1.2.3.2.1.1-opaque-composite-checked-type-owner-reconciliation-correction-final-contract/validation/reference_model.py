#!/usr/bin/env python3
from dataclasses import dataclass
from enum import IntEnum

class Admission(IntEnum):
    EXACT = 0
    WIDE = 1

@dataclass(frozen=True)
class Owner:
    producer: str
    identity: str
    admission: Admission
    def accepts_owner(self, actual):
        return self == actual or (
            self.admission == Admission.WIDE and
            actual.admission == Admission.EXACT and
            self.producer == actual.producer
        )
    def accepts_value(self, value):
        return self.producer == value.producer and (
            self.admission == Admission.WIDE or self.identity == value.identity
        )

@dataclass(frozen=True)
class OpaqueValue:
    producer: str
    identity: str
    payload: object

@dataclass(frozen=True)
class Checked:
    kind: str
    data: object = None
    def accepts(self, value):
        if self.kind == 'never': return False
        if self.kind == 'unit': return value == ('unit',)
        if self.kind == 'opaque': return isinstance(value, OpaqueValue) and self.data.accepts_value(value)
        if self.kind == 'tuple': return isinstance(value, tuple) and len(value) == len(self.data) and all(t.accepts(v) for t,v in zip(self.data,value))
        if self.kind == 'sequence': return isinstance(value, list) and all(self.data.accepts(v) for v in value)
        if self.kind == 'choice': return any(t.accepts(value) for t in self.data)
        if self.kind == 'option':
            tag, payload = value
            return tag == 'None' or (tag == 'Some' and self.data.accepts(payload))
        if self.kind == 'result':
            tag, payload = value
            return (tag == 'Ok' and self.data[0].accepts(payload)) or (tag == 'Err' and self.data[1].accepts(payload))
        raise AssertionError(self.kind)

def run():
    pa = Owner('P','A',Admission.EXACT)
    pb = Owner('P','B',Admission.EXACT)
    qa = Owner('Q','A',Admission.EXACT)
    pw = Owner('P','TOP',Admission.WIDE)
    pu = Owner('P','OTHER',Admission.WIDE)
    va = OpaqueValue('P','A',('unit',))
    vb = OpaqueValue('P','B',('unit',))
    vq = OpaqueValue('Q','A',('unit',))
    checks = [
      ('owner exact equal', pa.accepts_owner(pa)),
      ('owner exact identity mismatch', not pa.accepts_owner(pb)),
      ('owner exact producer mismatch', not pa.accepts_owner(qa)),
      ('owner wide exact same producer', pw.accepts_owner(pa)),
      ('owner wide exact other producer', not pw.accepts_owner(qa)),
      ('owner wide unequal wide', not pw.accepts_owner(pu)),
      ('value exact', pa.accepts_value(va)),
      ('value exact mismatch', not pa.accepts_value(vb)),
      ('value wide A', pw.accepts_value(va)),
      ('value wide B', pw.accepts_value(vb)),
      ('value wide other producer', not pw.accepts_value(vq)),
      ('tuple empty', Checked('tuple',()).accepts(())),
      ('choice empty', not Checked('choice',()).accepts(va)),
      ('sequence empty', Checked('sequence',Checked('opaque',pa)).accepts([])),
      ('sequence bad', not Checked('sequence',Checked('opaque',pa)).accepts([va,vb])),
      ('option none', Checked('option',Checked('never')).accepts(('None',None))),
      ('option some never', not Checked('option',Checked('never')).accepts(('Some',('unit',)))),
      ('result ok full', Checked('result',(Checked('opaque',pa),Checked('opaque',pb))).accepts(('Ok',va))),
      ('result err full', Checked('result',(Checked('opaque',pa),Checked('opaque',pb))).accepts(('Err',vb))),
      ('result wrong branch identity', not Checked('result',(Checked('opaque',pa),Checked('opaque',pb))).accepts(('Err',va))),
    ]
    failed=[name for name,ok in checks if not ok]
    for name,ok in checks: print(f'{name}: {"PASS" if ok else "FAIL"}')
    print(f'TOTAL={len(checks)} PASS={len(checks)-len(failed)} FAIL={len(failed)}')
    if failed: raise SystemExit(1)

if __name__ == '__main__': run()
