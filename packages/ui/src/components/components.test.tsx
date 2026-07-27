import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Button } from "./Button";
import { Input } from "./Input";
import { Progress } from "./Progress";
import { SegmentedControl } from "./SegmentedControl";

describe("QZip UI primitives", () => {
  it("disables a loading button", () => {
    render(<Button loading>保存</Button>);
    expect(screen.getByRole("button", { name: "保存" })).toBeDisabled();
  });

  it("connects an input error to the field", () => {
    render(<Input label="文件名" error="名称不能为空" />);
    expect(screen.getByRole("textbox", { name: "文件名" })).toHaveAttribute(
      "aria-invalid",
      "true"
    );
  });

  it("exposes determinate progress semantics", () => {
    render(<Progress label="压缩进度" value={68} />);
    expect(screen.getByRole("progressbar", { name: "压缩进度" })).toHaveAttribute(
      "aria-valuenow",
      "68"
    );
  });

  it("changes a segmented value", () => {
    const onValueChange = vi.fn();
    render(
      <SegmentedControl
        ariaLabel="格式"
        value="7z"
        onValueChange={onValueChange}
        options={[
          { value: "7z", label: "7Z" },
          { value: "zip", label: "ZIP" }
        ]}
      />
    );

    screen.getByRole("radio", { name: "ZIP" }).click();
    expect(onValueChange).toHaveBeenCalledWith("zip");
  });
});
