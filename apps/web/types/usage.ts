export interface UsageGraphPoint {
  period: string;
  inputs: number;
  outputs: number;
  cache: number;
}

export interface UsageShared {
  inputs: number;
  outputs: number;
  cache: number;
}

export interface UsageGraphResponse {
  points: UsageGraphPoint[];
  shared: UsageShared;
}

export type Granularity =
  | "15min"
  | "30min"
  | "1hr"
  | "4hr"
  | "12hr"
  | "daily"
  | "weekly"
  | "monthly";
