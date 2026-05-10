# Architecture Pack Boundary — Intentional Failure

Model a forbidden adapter-to-domain dependency, then run architecture validation.
Expected result: validation emits a forbidden-dependency finding with remediation
to move the dependency behind a port/adapter boundary.
