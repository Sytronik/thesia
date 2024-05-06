import {createContext} from "react";
import Store from "./Store";

export const StoreContext = createContext<Store>({} as Store);

export const StoreProvider = StoreContext.Provider;
