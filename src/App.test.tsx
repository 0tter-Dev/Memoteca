import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import App from "./App";

describe("App", () => {
  it("mostra o nome do projeto como título principal", () => {
    render(<App />);

    expect(
      screen.getByRole("heading", { level: 1, name: "Memoteca" }),
    ).toBeInTheDocument();
  });

  it("não usa o nome provisório do projeto", () => {
    const { container } = render(<App />);

    expect(container.textContent?.toLowerCase()).not.toContain("ollamaui");
  });
});
