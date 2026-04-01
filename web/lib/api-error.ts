import type { ApiResponse, ErrorCode, ErrorResponse } from "@/types/api";
import { redirect } from "next/navigation";

const STATUS_MAP: Partial<Record<ErrorCode, string | (() => void)>> = {
  INVALID_INPUT: "The provided input is invalid.",
  UNAUTHORIZED: "You are not authenticated. Please log in.",
  INVALID_TOKEN: () => {
    redirect("/logout");
  },
  INVALID_CLAIMS: "Your session is invalid. Please log in again.",
  BAD_SIGNATURE: "Your session could not be verified. Please log in again.",
  FORBIDDEN: "You do not have permission to perform this action.",
  NOT_FOUND: "The requested resource was not found.",
  ALREADY_EXISTS: "This resource already exists.",
  CONFLICT: "This action conflicts with the current state. Please refresh and try again.",
  DUPLICATE_ENTRY: "A duplicate entry was found. Please use a unique value.",
  INTERNAL_ERROR: "Something went wrong. Please try again later.",
};

export function isErrorResponse<T>(res: ApiResponse<T>): res is ErrorResponse {
  return "error" in res;
}

export function handleApiError(code: string, fallbackMessage: string): ErrorResponse {
  const handler = STATUS_MAP[code as ErrorCode];

  if (handler) {
    if (typeof handler === "function") {
      handler();
    }
    if (typeof handler === "string") {
      return { error: { message: handler, code: code as ErrorCode } };
    }
  }

  return { error: { message: fallbackMessage, code: code as ErrorCode } };
}
