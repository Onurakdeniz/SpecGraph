# Architecture Pack Boundary — Happy Path

1. Validate `docs/ontology-packs/ddd-backend.yaml`.
2. Install the pack through `sg ontology install-pack`.
3. Run architecture validation/CI so layer and port facts satisfy the pack.

Expected result: pack validation passes and install writes a receipt through the
Operation Runtime.
