import { sendRequest } from "../bridge/client";
import { createDocumentSession } from "./sync/session";

/** App-wide document session. Tests construct their own. */
export const documentSession = createDocumentSession({ send: sendRequest });
