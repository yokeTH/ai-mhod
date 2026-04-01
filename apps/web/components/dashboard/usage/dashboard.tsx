"use client";

import { useEffect, useMemo, useState } from "react";
import { ChartBarStacked } from "@/components/dashboard/usage/chart";
import type { DataKey } from "@/components/dashboard/usage/chart";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import { Checkbox } from "@/components/ui/checkbox";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import type { UsageGraphPoint, UsageShared } from "@/types/usage";
import { CalendarIcon } from "lucide-react";
import { useQueryStates } from "nuqs";
import { usageParsers, GRANULARITY_VALUES } from "@/app/dashboard/usage/params";
import type { DateRange } from "react-day-picker";

const GRANULARITIES: { value: (typeof GRANULARITY_VALUES)[number]; label: string }[] = [
  { value: "15min", label: "15 min" },
  { value: "30min", label: "30 min" },
  { value: "1hr", label: "1 hour" },
  { value: "4hr", label: "4 hours" },
  { value: "12hr", label: "12 hours" },
  { value: "daily", label: "Daily" },
  { value: "weekly", label: "Weekly" },
  { value: "monthly", label: "Monthly" },
];

const VIEW_OPTIONS: { key: DataKey; label: string }[] = [
  { key: "inputs", label: "Input Tokens" },
  { key: "outputs", label: "Output Tokens" },
  { key: "cache", label: "Cache Tokens" },
];

function formatDate(date: Date): string {
  return date.toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
  });
}

function toDateString(date: Date): string {
  return date.toISOString();
}

const SHARED_CARDS: { key: keyof UsageShared; label: string }[] = [
  { key: "inputs", label: "Input Tokens" },
  { key: "outputs", label: "Output Tokens" },
  { key: "cache", label: "Cache Tokens" },
];

export function UsageDashboard({ points, shared, models }: { points: UsageGraphPoint[]; shared: UsageShared; models: string[] }) {
  const [params, setParams] = useQueryStates(usageParsers, { shallow: false });

  const defaults = useMemo(() => {
    const today = new Date();
    const sevenDaysAgo = new Date();
    sevenDaysAgo.setDate(sevenDaysAgo.getDate() - 7);
    return { from: toDateString(sevenDaysAgo), to: toDateString(today) };
  }, []);

  const fromStr = params.from ?? defaults.from;
  const toStr = params.to ?? defaults.to;
  const granularity = params.granularity ?? "1hr";
  const selectedModel = params.model ?? "";

  const [calRange, setCalRange] = useState<DateRange | undefined>(() => ({
    from: new Date(fromStr),
    to: new Date(toStr),
  }));

  const [visibleKeys, setVisibleKeys] = useState<DataKey[]>(["inputs", "outputs"]);

  useEffect(() => {
    setCalRange({ from: new Date(fromStr), to: new Date(toStr) });
  }, [fromStr, toStr]);

  function handleSelect(range: DateRange | undefined) {
    setCalRange(range);
    if (range?.from && range.to) {
      setParams({
        from: toDateString(range.from),
        to: toDateString(range.to),
      });
    }
  }

  function toggleKey(key: DataKey) {
    setVisibleKeys((prev) =>
      prev.includes(key) ? prev.filter((k) => k !== key) : [...prev, key]
    );
  }

  const dateLabel = calRange?.from
    ? calRange.to
      ? `${formatDate(calRange.from)} – ${formatDate(calRange.to)}`
      : formatDate(calRange.from)
    : "Pick a date range";

  return (
    <div className="space-y-4">
      <div className="grid grid-cols-3 gap-4">
        {SHARED_CARDS.map(({ key, label }) => (
          <Card key={key}>
            <CardHeader className="pb-2">
              <CardTitle className="text-sm font-medium text-muted-foreground">{label}</CardTitle>
            </CardHeader>
            <CardContent>
              <p className="text-2xl font-bold">{shared[key].toLocaleString()}</p>
            </CardContent>
          </Card>
        ))}
      </div>

      <div className="flex flex-wrap items-center gap-3">
        <Popover>
          <PopoverTrigger asChild>
            <Button variant="outline" className="justify-start gap-2 text-sm font-normal">
              <CalendarIcon className="size-4" />
              {dateLabel}
            </Button>
          </PopoverTrigger>
          <PopoverContent className="w-auto p-0" align="start">
            <Calendar
              mode="range"
              selected={calRange}
              onSelect={handleSelect}
              numberOfMonths={1}
              disabled={{ after: new Date() }}
            />
          </PopoverContent>
        </Popover>

        <Select value={granularity} onValueChange={(v) => setParams({ granularity: v as (typeof GRANULARITY_VALUES)[number] })}>
          <SelectTrigger>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {GRANULARITIES.map((g) => (
              <SelectItem key={g.value} value={g.value}>
                {g.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>

        {models.length > 0 && (
          <Select value={selectedModel || "all"} onValueChange={(v) => setParams({ model: v === "all" ? null : v })}>
            <SelectTrigger className="w-[200px]">
              <SelectValue placeholder="All Models" />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="all">All Models</SelectItem>
              {models.map((m) => (
                <SelectItem key={m} value={m}>
                  {m}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}

        <div className="flex items-center gap-4">
          {VIEW_OPTIONS.map(({ key, label }) => (
            <label key={key} className="flex items-center gap-2 text-sm">
              <Checkbox
                checked={visibleKeys.includes(key)}
                onCheckedChange={() => toggleKey(key)}
              />
              {label}
            </label>
          ))}
        </div>
      </div>

      <ChartBarStacked points={points} visibleKeys={visibleKeys} />
    </div>
  );
}
