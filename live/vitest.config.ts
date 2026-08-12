import { defineWorkersConfig, readD1Migrations } from "@cloudflare/vitest-pool-workers/config";
import { fileURLToPath } from "node:url";

export default defineWorkersConfig(async () => {
  const migrationsDir = fileURLToPath(new URL("./migrations", import.meta.url));
  const migrations = await readD1Migrations(migrationsDir);
  return {
    test: {
      include: ["test/**/*.spec.ts"],
      setupFiles: ["test/apply-migrations.ts"],
      poolOptions: {
        workers: {
          wrangler: { configPath: "./wrangler.jsonc" },
          miniflare: {
            // Exposed to the setup file so it can apply migrations to the
            // per-test D1 database.
            bindings: { TEST_MIGRATIONS: migrations },
          },
        },
      },
    },
  };
});
