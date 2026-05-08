import assert from "node:assert/strict";
import test from "node:test";
import { requestPasswordReset } from "../../src/identity/password-reset.js";

test("test:identity/password-reset/generic-response-existing", () => {
  assert.deepEqual(requestPasswordReset("user@example.com"), {
    status: 202,
    body: {
      message: "If an account exists, a password reset email will be sent."
    }
  });
});

test("test:identity/password-reset/generic-response-unknown", () => {
  assert.deepEqual(requestPasswordReset("unknown@example.com"), {
    status: 202,
    body: {
      message: "If an account exists, a password reset email will be sent."
    }
  });
});
