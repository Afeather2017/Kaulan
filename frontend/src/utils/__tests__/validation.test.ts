import { describe, expect, it } from "vitest";
import {
  validateRequiredServerUrl,
  validateServerUrl,
} from "@/utils/validation";

describe("validateServerUrl", () => {
  it("allows empty input for default localhost settings behavior", () => {
    expect(validateServerUrl("")).toEqual({ valid: true });
    expect(validateServerUrl("   ")).toEqual({ valid: true });
  });

  it("accepts a normal device address", () => {
    expect(validateServerUrl("192.168.1.10:2080")).toEqual({ valid: true });
  });
});

describe("validateRequiredServerUrl", () => {
  it("rejects empty input for manual device entry", () => {
    expect(validateRequiredServerUrl("")).toEqual({
      valid: false,
      error: "请输入设备地址",
    });
    expect(validateRequiredServerUrl("   ")).toEqual({
      valid: false,
      error: "请输入设备地址",
    });
  });

  it("reuses standard server URL validation for non-empty values", () => {
    expect(validateRequiredServerUrl("192.168.1.10:2080")).toEqual({
      valid: true,
    });
  });
});
