# Validation contract

Run from the package parent:

```text
python arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract/tools/validate_package.py arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract
python arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract/tools/validate_package.py arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract.zip
unzip -t arcweft-lang-01.5.1.1.2.1.1.1.1-checked-match-coverage-runtime-need-identity-and-awbc-allocation-correction-final-contract.zip
```

The validator checks safe archive paths and one exact root, required artifacts,
`OPEN_QUESTIONS.md == "none\n"`, current request bytes/hash/blob, predecessor
request hash metadata, all 23 decision rows, all 64 allocated opcode bytes,
reserved ranges, function kinds/tombstones, flag bits/mask/constraints, version
1 fixation, varint/single-buffer/tensor/no-usize rules, every current TypeKind,
every pattern family, every current identity consumer category, persisted-HIR
absence, required test rows, exact source ranges, structural absence rules, and
internal SHA-256 manifests.

`VALIDATION_OUTPUT.txt` is generated only after directory, archive, ZIP
integrity, and independent hash validation pass.
