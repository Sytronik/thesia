import {useContext} from "react";
import Store from "renderer/Store";
import {StoreContext} from "../contexts";

const useStore = (): Store => useContext(StoreContext);

export default useStore;
