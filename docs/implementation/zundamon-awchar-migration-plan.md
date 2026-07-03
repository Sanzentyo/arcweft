# Zundamon `.awchar` migration plan

1. Keep the source PSD outside runtime artifacts.
2. Run importer:

   ```bash
   arcw import psd-character art/zundamon.psd \
     --character character.zundamon \
     --default-look normal \
     --output assets/zundamon.awchar \
     --force \
     --json
   ```

3. Ensure the PSD uses top-level groups:

   ```text
   part:body
   part:eyes
   part:mouth
   look:normal      # marker layers: body=default, eyes=normal, mouth=neutral
   look:smile       # marker layers: body=default, eyes=smile, mouth=smile
   ```

4. Add the package to the launch profile:

   ```toml
   default = "dev"

   [profiles.dev]
   kind = "game"
   source = "src/main.arcw"
   character_manifests = ["assets/zundamon.awchar"]
   ```

5. Replace flat PNG swaps with typed look selection:

   ```arcw
   show(@character.zundamon, look = .normal)
   show(@character.zundamon, look = .smile)
   ```

6. Validate stable anchoring by comparing Agent observe bboxes for both looks.  They must be identical because they derive from the manifest canvas and anchor.

The synthetic sample in this zip demonstrates the target layout.  Its PNG snapshots under `validation/visual-evidence` are evidence only and must not be published as the product character runtime path.
