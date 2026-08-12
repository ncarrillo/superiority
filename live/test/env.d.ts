declare module "cloudflare:test" {
  // Extends the generated Env with the test-only migrations binding.
  interface ProvidedEnv extends Env {
    TEST_MIGRATIONS: D1Migration[];
  }
}
