import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { App } from "../src/App";

const demoCatalogueUrl = "http://127.0.0.1:4242/metadata";

async function discoverDemoCatalogue() {
  render(<App />);

  fireEvent.change(screen.getByLabelText(/catalogue url/i), {
    target: { value: demoCatalogueUrl },
  });
  fireEvent.click(screen.getByRole("button", { name: /^discover$/i }));

  await waitFor(() => expect(screen.getByText(/semantic asset overview/i)).toBeInTheDocument());
}

describe("App workbench", () => {
  it("renders the first-visit workbench without persistent storage", () => {
    const localStorageSpy = vi.spyOn(Storage.prototype, "setItem");

    render(<App />);

    expect(screen.getByRole("heading", { name: /semantic discovery workbench/i })).toBeInTheDocument();
    expect(screen.getByLabelText(/catalogue url/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/session-only bearer token/i)).toBeInTheDocument();
    expect(screen.getByText(/validation not yet run/i)).toBeInTheDocument();
    expect(screen.getByText(/session-only history/i)).toBeInTheDocument();
    expect(
      within(screen.getByLabelText(/catalog source shortcuts/i)).getByRole("button", {
        name: /bundled registry relay discovery/i,
      }),
    ).toBeInTheDocument();
    const workspaceTabs = screen.getByRole("navigation", { name: /workspace tabs/i });
    expect(within(workspaceTabs).getByRole("button", { name: /overview/i })).toBeInTheDocument();
    expect(within(workspaceTabs).getByRole("button", { name: /^semantic assets$/i })).toBeInTheDocument();
    expect(within(workspaceTabs).getByRole("button", { name: /evidence/i })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: /registry relay publisher profile/i })).not.toBeInTheDocument();
    expect(screen.queryByText(/^registry relay publisher profile$/i)).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^missing$/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^list$/i })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /^map$/i })).not.toBeInTheDocument();
    expect(screen.queryByLabelText(/evidence sources/i)).not.toBeInTheDocument();
    expect(localStorageSpy).not.toHaveBeenCalled();

    localStorageSpy.mockRestore();
  });

  it("discovers a demo catalogue and hides publisher-specific fields in core metadata mode", async () => {
    await discoverDemoCatalogue();

    expect(screen.getAllByRole("heading", { name: /government demo registry relay/i }).length).toBeGreaterThan(0);
    expect(screen.getByText(/decision view/i)).toBeInTheDocument();
    expect(screen.getByText(/which semantic assets Atlas can register/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/semantic asset overview summary/i)).toBeInTheDocument();
    expect(screen.getAllByText(/^semantic assets$/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/credential-gated/i)).toBeInTheDocument();
    expect(screen.getAllByText(/readiness checks/i).length).toBeGreaterThan(0);
    expect(screen.getByText(/publisher-specific metadata is excluded/i)).toBeInTheDocument();
    expect(screen.queryByLabelText(/evidence sources/i)).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^evidence$/i }));
    expect(screen.getByText(/publisher-specific metadata is hidden in core metadata mode/i)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: /^semantic assets$/i }));
    expect(screen.getByRole("button", { name: /^list$/i })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^Benefits Casework$/ }));

    const detailPanel = screen.getByLabelText(/inspector/i);
    expect(within(detailPanel).getByText(/dcat:Dataset -> dcterms:title/i)).toHaveTextContent("dcterms:title");

    fireEvent.click(screen.getByRole("button", { name: /raw rdf \/ json-ld/i }));
    expect(screen.getAllByText(/dcat:Dataset/i).length).toBeGreaterThan(0);
  });

  it("groups evidence as recognized, publisher-specific, and follow-up artifacts", async () => {
    render(<App />);

    fireEvent.change(screen.getByLabelText(/catalogue url/i), {
      target: { value: demoCatalogueUrl },
    });
    fireEvent.click(screen.getByRole("button", { name: /publisher metadata/i }));
    fireEvent.click(screen.getByRole("button", { name: /^discover$/i }));

    await waitFor(() => expect(screen.getByText(/semantic asset overview/i)).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: /^evidence$/i }));

    expect(screen.getByRole("heading", { name: /^recognized metadata artifacts$/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /^publisher-specific metadata$/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /^follow-up or unparsed artifacts$/i })).toBeInTheDocument();
    expect(screen.queryByLabelText(/evidence sources/i)).not.toBeInTheDocument();
    expect(screen.getAllByText(/publisher-specific/i).length).toBeGreaterThan(0);
  });

  it("uses Semantic assets as the asset browser and reveals access methods after selecting a semantic asset", async () => {
    await discoverDemoCatalogue();

    fireEvent.click(screen.getByRole("button", { name: /^semantic assets$/i }));

    const workspace = screen.getByLabelText(/center workspace/i);
    expect(within(workspace).getAllByText(/^semantic assets$/i).length).toBeGreaterThan(0);
    expect(within(workspace).getByRole("button", { name: /^inspect source metadata$/i })).toBeInTheDocument();
    expect(within(workspace).getByRole("heading", { name: /^registerable semantic assets$/i })).toBeInTheDocument();
    expect(within(workspace).getByRole("heading", { name: /^access methods$/i })).toBeInTheDocument();
    expect(within(workspace).getByText(/^no dataset selected$/i)).toBeInTheDocument();
    expect(within(workspace).getByText(/choose a semantic asset/i)).toBeInTheDocument();
    expect(within(workspace).queryByRole("heading", { name: /^catalog$/i })).not.toBeInTheDocument();
    expect(within(workspace).queryByText(/services and distributions/i)).not.toBeInTheDocument();
    expect(within(workspace).queryByText(/access rights/i)).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /^Benefits Casework$/ }));

    expect(within(screen.getByLabelText(/selected semantic asset path/i)).getByText(/^Benefits Casework$/i)).toBeInTheDocument();
    expect(within(workspace).queryByText(/^no dataset selected$/i)).not.toBeInTheDocument();
    expect(within(workspace).getByText(/published dcat distributions and data services/i)).toBeInTheDocument();
    expect(within(workspace).getByText(/Benefit Case REST access service/i)).toBeInTheDocument();

    fireEvent.click(within(workspace).getByRole("button", { name: /^Benefit Case REST access service$/ }));
    expect(within(screen.getByLabelText(/selected semantic asset path/i)).getByText(/^Benefit Case REST access service$/i)).toBeInTheDocument();

    const detailPanel = screen.getByLabelText(/inspector/i);
    expect(within(detailPanel).getByRole("heading", { name: /^Benefit Case REST access service$/i })).toBeInTheDocument();
    expect(within(detailPanel).queryByText(/services and distributions/i)).not.toBeInTheDocument();
  });
});
