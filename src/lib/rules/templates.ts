// Starting points for a new rule: the usual "mail from … into a folder",
// "mail to … gets a label", "subject … into a folder", filled in from a
// sample message when there is one.

import type { Rule } from "#tauri/types";
import { buildAction, buildCondition, type ConditionView } from "./view";

export type TemplateKind = "sender" | "recipient" | "subject";

const CONDITIONS: Record<TemplateKind, Omit<ConditionView, "values">> = {
  sender: { field: "from", header: "", part: "domain", operator: "is" },
  recipient: { field: "recipients", header: "", part: "header", operator: "contains" },
  subject: { field: "subject", header: "", part: "header", operator: "contains" },
};

export function templateRule(kind: TemplateKind, name: string, value = ""): Rule {
  return {
    name,
    enabled: true,
    conditions: { join: "any", items: [buildCondition({ ...CONDITIONS[kind], values: [value] })] },
    actions: [buildAction(kind === "recipient" ? "addFlags" : "move")],
    elsif: [],
    otherwise: null,
  };
}
