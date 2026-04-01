import { auth } from "@/lib/auth";
import { headers } from "next/headers";
import { notFound, redirect } from "next/navigation";

import { TokenCard } from "@/components/dev/token-card";

function decodeJwtPayload(token: string) {
  try {
    const base64 = token.split(".")[1];
    return JSON.parse(Buffer.from(base64, "base64url").toString());
  } catch {
    return null;
  }
}

export default async function DevPage() {
  if (process.env.NODE_ENV === "production") notFound();

  const session = await auth.api.getSession({
    headers: await headers(),
  });

  if (!session) redirect("/login");

  let accessToken: string | null = null;
  let tokenPayload: Record<string, unknown> | null = null;
  let error: string | null = null;

  try {
    const result = await auth.api.getAccessToken({
      body: { providerId: "keycloak" },
      headers: await headers(),
    });
    accessToken = result.accessToken;
    tokenPayload = decodeJwtPayload(accessToken);
  } catch (e) {
    error = e instanceof Error ? e.message : "Failed to get access token";
  }

  return (
    <div className="mx-auto max-w-3xl space-y-6 p-6">
      <h1 className="text-2xl font-semibold">Dev Tools</h1>

      <TokenCard
        title="Session"
        data={{
          userId: session.user.id,
          name: session.user.name,
          email: session.user.email,
          image: session.user.image,
          sessionToken: session.session.token,
          expiresAt: session.session.expiresAt,
        }}
      />

      {error ? (
        <div className="border-destructive/50 bg-destructive/10 text-destructive rounded-xl border p-4 text-sm">
          {error}
        </div>
      ) : (
        <>
          <TokenCard title="Access Token" copyValue={accessToken!} data={accessToken!} />

          {tokenPayload && (
            <TokenCard
              title="Token Claims"
              data={{
                sub: tokenPayload.sub,
                scope: tokenPayload.scope,
                iss: tokenPayload.iss,
                aud: tokenPayload.aud,
                exp: tokenPayload.exp
                  ? new Date((tokenPayload.exp as number) * 1000).toISOString()
                  : undefined,
                iat: tokenPayload.iat
                  ? new Date((tokenPayload.iat as number) * 1000).toISOString()
                  : undefined,
                azp: tokenPayload.azp,
                realm_access: tokenPayload.realm_access,
                resource_access: tokenPayload.resource_access,
                preferred_username: tokenPayload.preferred_username,
                email_verified: tokenPayload.email_verified,
              }}
            />
          )}
        </>
      )}
    </div>
  );
}
