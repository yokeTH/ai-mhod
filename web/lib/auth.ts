import { env } from "@/env";
import { betterAuth } from "better-auth";
import { genericOAuth } from "better-auth/plugins";

export const auth = betterAuth({
  session: {
    cookieCache: {
      enabled: true,
      maxAge: 7 * 24 * 60 * 60,
      strategy: "jwe",
    },
  },
  account: {
    storeStateStrategy: "cookie",
    storeAccountCookie: true,
  },
  plugins: [
    genericOAuth({
      config: [
        {
          providerId: "keycloak",
          clientId: env.KEYCLOAK_CLIENT_ID,
          clientSecret: env.KEYCLOAK_CLIENT_SECRET,
          discoveryUrl: `${env.KEYCLOAK_ISSUER}/.well-known/openid-configuration`,
          scopes: ["openid", "profile", "email", "offline_access"],
          accessType: "offline",
          pkce: true,
        },
      ],
    }),
  ],
});
