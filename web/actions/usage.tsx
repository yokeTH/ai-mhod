"use server";

import { apiGet } from "@/lib/api";
import type { UsageGraphResponse, Granularity } from "@/types/usage";

interface UsageGraphParams {
  from?: string;
  to?: string;
  granularity?: Granularity;
  model?: string;
}

export async function getUsageGraph(params?: UsageGraphParams) {
  const searchParams = new URLSearchParams();
  if (params?.from) searchParams.set("from", params.from);
  if (params?.to) searchParams.set("to", params.to);
  if (params?.granularity) searchParams.set("granularity", params.granularity);
  if (params?.model) searchParams.set("model", params.model);

  const query = searchParams.toString();
  return apiGet<UsageGraphResponse>(`/dashboard/usage/graph${query ? `?${query}` : ""}`);
}

export async function getUsageModels() {
  return apiGet<string[]>("/dashboard/usage/models");
}
