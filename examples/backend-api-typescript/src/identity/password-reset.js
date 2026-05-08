export function requestPasswordReset(_email) {
  return {
    status: 202,
    body: {
      message: "If an account exists, a password reset email will be sent."
    }
  };
}
