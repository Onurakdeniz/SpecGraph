# Coding-Agent Governed Edit — Happy Path

1. Initialize project, project profile, and module baseline.
2. Import or create `AUTH-001`, bind the branch, and run `sg workflow plan`.
3. Ask for `function:requestPasswordReset` and run:

   ```bash
   sg workflow code-plan \
     --spec AUTH-001 \
     --action implementation \
     --wants function:requestPasswordReset \
     --file src/identity/password-reset.rs
   ```

4. If the object is missing, declare it before editing:

   ```bash
   sg code declare-object \
     --spec AUTH-001 \
     --module Identity \
     --kind function \
     --name requestPasswordReset \
     --layer application \
     --file src/identity/password-reset.rs
   ```

5. Generate the action graph and edit only the permitted file/symbol.
6. Run strict indexing and reconciliation:

   ```bash
   sg code index --changed-file src/identity/password-reset.rs --strict
   ```

7. Validate and commit with scoped evidence:

   ```bash
   sg git validate-message \
     --message-file .git/COMMIT_EDITMSG \
     --changed-file src/identity/password-reset.rs \
     --changed-symbol requestPasswordReset
   ```

Expected result: the agent receives an edit permit only after the code object is
declared, `sg code index --strict` reconciles the observed symbol to the
declaration via `CodeObject.Reconcile`, and commit validation accepts only the
allowed file and symbol.
