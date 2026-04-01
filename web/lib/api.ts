"use server";

import { env } from "@/env";
import { auth } from "@/lib/auth";
import { headers as nextHeaders } from "next/headers";
import type { ApiResponse } from "@/types/api";

import { handleApiError, isErrorResponse } from "./api-error";

interface RequestOptions {
  method?: string;
  body?: unknown;
}

export async function api<T>(path: string, options?: RequestOptions): Promise<ApiResponse<T>> {
  const { accessToken } = await auth.api.getAccessToken({
    body: { providerId: "keycloak" },
    headers: await nextHeaders(),
  });

  const headers: Record<string, string> = {
    Authorization: `Bearer ${accessToken}`,
  };

  if (options?.body !== undefined) {
    headers["Content-Type"] = "application/json";
  }

  const res = await fetch(`${env.API_BASE_URL}${path}`, {
    method: options?.method,
    headers,
    body: options?.body !== undefined ? JSON.stringify(options.body) : undefined,
  });

  const json: ApiResponse<T> = await res.json();

  if (isErrorResponse(json)) {
    return handleApiError(json.error.code, json.error.message);
  }

  return json;
}

export async function apiGet<T>(path: string) {
  return api<T>(path);
}

export async function apiMutate<T = never>(path: string, method: string, body?: unknown) {
  return api<T>(path, { method, body });
}
