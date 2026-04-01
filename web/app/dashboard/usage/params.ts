import {
  createSearchParamsCache,
  parseAsString,
  parseAsStringLiteral,
} from "nuqs/server";

export const GRANULARITY_VALUES = [
  "15min",
  "30min",
  "1hr",
  "4hr",
  "12hr",
  "daily",
  "weekly",
  "monthly",
] as const;

export const usageParsers = {
  from: parseAsString,
  to: parseAsString,
  granularity: parseAsStringLiteral(GRANULARITY_VALUES).withDefault("1hr"),
};

export const usageSearchParamsCache = createSearchParamsCache(usageParsers);
