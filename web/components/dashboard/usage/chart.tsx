"use client";

import { Bar, BarChart, CartesianGrid, XAxis } from "recharts";

import { Card, CardContent } from "@/components/ui/card";
import {
  ChartContainer,
  ChartLegend,
  ChartLegendContent,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart";
import { UsageGraphPoint } from "@/types/usage";

export type DataKey = "inputs" | "outputs" | "cache";

const allChartConfig = {
  inputs: { label: "Input Tokens", color: "var(--chart-1)" },
  outputs: { label: "Output Tokens", color: "var(--chart-2)" },
  cache: { label: "Cache Tokens", color: "var(--chart-3)" },
} satisfies ChartConfig;

const DATA_KEYS: DataKey[] = ["inputs", "outputs", "cache"];

export function ChartBarStacked({
  points,
  visibleKeys = ["inputs", "outputs"],
}: {
  points: UsageGraphPoint[];
  visibleKeys?: DataKey[];
}) {
  const activeKeys = DATA_KEYS.filter((k) => visibleKeys.includes(k));

  const chartConfig = Object.fromEntries(
    activeKeys.map((key) => [key, allChartConfig[key]])
  ) as ChartConfig;

  return (
    <Card>
      <CardContent className="pt-6">
        <ChartContainer config={chartConfig}>
          <BarChart accessibilityLayer data={points}>
            <CartesianGrid vertical={false} />
            <XAxis
              dataKey="period"
              tickLine={false}
              tickMargin={10}
              axisLine={false}
              tickFormatter={(value) => {
                const d = new Date(value);
                return isNaN(d.getTime())
                  ? value
                  : d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
              }}
            />
            <ChartTooltip content={<ChartTooltipContent hideLabel />} />
            <ChartLegend content={<ChartLegendContent />} />
            {activeKeys.map((key, i) => {
              const isBottom = i === 0;
              const isTop = i === activeKeys.length - 1;
              const radius: [number, number, number, number] =
                activeKeys.length === 1
                  ? [4, 4, 4, 4]
                  : isBottom
                    ? [0, 0, 4, 4]
                    : isTop
                      ? [4, 4, 0, 0]
                      : [0, 0, 0, 0];
              return (
                <Bar
                  key={key}
                  dataKey={key}
                  stackId="a"
                  fill={`var(--color-${key})`}
                  radius={radius}
                />
              );
            })}
          </BarChart>
        </ChartContainer>
      </CardContent>
    </Card>
  );
}
