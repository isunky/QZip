import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { HomePage } from "./HomePage";

describe("HomePage", () => {
  it("shows the primary QZip entry points", () => {
    render(<HomePage onUnavailable={vi.fn()} />);

    expect(screen.getByRole("heading", { name: "将文件拖到这里" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "压缩文件" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "打开压缩包" })).toBeInTheDocument();
  });

  it("keeps the incomplete file workflow explicit", () => {
    const onUnavailable = vi.fn();
    render(<HomePage onUnavailable={onUnavailable} />);

    fireEvent.click(screen.getByRole("button", { name: "压缩文件" }));

    expect(onUnavailable).toHaveBeenCalledOnce();
  });
});
