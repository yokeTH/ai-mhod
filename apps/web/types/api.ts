export type ErrorCode =
  | "INVALID_INPUT"
  | "UNAUTHORIZED"
  | "INVALID_TOKEN"
  | "INVALID_CLAIMS"
  | "BAD_SIGNATURE"
  | "FORBIDDEN"
  | "NOT_FOUND"
  | "ALREADY_EXISTS"
  | "CONFLICT"
  | "DUPLICATE_ENTRY"
  | "INTERNAL_ERROR";

export type ApiResponse<T> = SuccessResponse<T> | ErrorResponse;

export interface SuccessResponse<T> {
  data: T;
  pagination: {
    page: number;
    last: number;
    limit: number;
    total: number;
  } | null;
}

export interface ErrorResponse {
  error: {
    message: string;
    code: ErrorCode;
  };
}
