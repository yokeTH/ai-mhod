import { createEnv } from "@t3-oss/env-nextjs";
import { z } from "zod";

export const env = createEnv({
  server: {
    API_BASE_URL: z.url(),
    KEYCLOAK_CLIENT_ID: z.string(),
    KEYCLOAK_CLIENT_SECRET: z.string(),
    KEYCLOAK_ISSUER: z.url(),
  },
  client: {},
  runtimeEnv: {
    API_BASE_URL: process.env.API_BASE_URL,
    KEYCLOAK_CLIENT_ID: process.env.KEYCLOAK_CLIENT_ID,
    KEYCLOAK_CLIENT_SECRET: process.env.KEYCLOAK_CLIENT_SECRET,
    KEYCLOAK_ISSUER: process.env.KEYCLOAK_ISSUER,
  },
});
