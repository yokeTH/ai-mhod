import { Suspense } from "react";
import { getUsageGraph } from "@/actions/usage";
import { UsageDashboard } from "@/components/dashboard/usage/dashboard";
import { isErrorResponse } from "@/lib/api-error";
import { usageSearchParamsCache } from "./params";
import type { SearchParams } from "nuqs/server";

export default async function UsagePage({
  searchParams,
}: {
  searchParams: Promise<SearchParams>;
}) {
  const { from: fromParam, to: toParam, granularity } =
    await usageSearchParamsCache.parse(searchParams);

  const today = new Date();
  const sevenDaysAgo = new Date();
  sevenDaysAgo.setDate(sevenDaysAgo.getDate() - 7);

  const from = fromParam ?? sevenDaysAgo.toISOString();
  const to = toParam ?? today.toISOString();

  const data = await getUsageGraph({ from, to, granularity });

  if (isErrorResponse(data)) {
    return "GET DATA ERROR";
  }

  return (
    <Suspense>
      <UsageDashboard points={data.data.points} shared={data.data.shared} />
    </Suspense>
  );
}
