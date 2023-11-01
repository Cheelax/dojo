import { useComponentValue } from "@latticexyz/react";
import { EntityIndex } from "@latticexyz/recs";
import { useEffect, useState } from "react";
import "./App.css";
import { useDojo } from "./DojoContext";
import { Direction } from "./dojo/createSystemCalls";

function App() {
    const {
        setup: {
            systemCalls: { spawn, move },
            components: { Moves, Position },
        },
        account: {
            create,
            list,
            select,
            account,
            isDeploying,
            clear,
            copyToClipboard,
            applyFromClipboard,
        },
    } = useDojo();

    const [clipboardStatus, setClipboardStatus] = useState({
        message: "",
        isError: false,
    });

    // entity id - this example uses the account address as the entity id
    const entityId = account.address.toString();

    // get current component values
    const position = useComponentValue(Position, entityId as EntityIndex);
    const moves = useComponentValue(Moves, entityId as EntityIndex);

    const handleRestoreBurners = async () => {
        try {
            await applyFromClipboard();
            setClipboardStatus({
                message: "Burners restored successfully!",
                isError: false,
            });
        } catch (error) {
            setClipboardStatus({
                message: `Failed to restore burners from clipboard`,
                isError: true,
            });
        }
    };

    useEffect(() => {
        // Clear message after 3 seconds
        if (clipboardStatus.message) {
            const timer = setTimeout(() => {
                setClipboardStatus({ message: "", isError: false });
            }, 3000);

            return () => clearTimeout(timer);
        }
    }, [clipboardStatus.message]);

    return (
        <>
            <button onClick={create}>
                {isDeploying ? "deploying burner" : "create burner"}
            </button>
            {list().length > 0 && (
                <button onClick={async () => await copyToClipboard()}>
                    Save Burners to Clipboard
                </button>
            )}
            <button onClick={handleRestoreBurners}>
                Restore Burners from Clipboard
            </button>
            {clipboardStatus.message && (
                <div className={clipboardStatus.isError ? "error" : "success"}>
                    {clipboardStatus.message}
                </div>
            )}

            <div className="card">
                select signer:{" "}
                <select
                    value={account ? account.address : ""}
                    onChange={(e) => select(e.target.value)}
                >
                    {list().map((account, index) => {
                        return (
                            <option value={account.address} key={index}>
                                {account.address}
                            </option>
                        );
                    })}
                </select>
                <div>
                    <button onClick={() => clear()}>Clear burners</button>
                </div>
            </div>
            <div className="card">
                <button onClick={() => spawn(account)}>Spawn</button>
                <div>
                    Moves Left:{" "}
                    {moves ? `${moves["remaining"]}` : "Need to Spawn"}
                </div>
                <div>
                    Position:{" "}
                    {position
                        ? `${position["x"]}, ${position["y"]}`
                        : "Need to Spawn"}
                </div>
            </div>
            <div className="card">
                <button onClick={() => move(account, Direction.Up)}>
                    Move Up
                </button>{" "}
                <br />
                <button onClick={() => move(account, Direction.Left)}>
                    Move Left
                </button>
                <button onClick={() => move(account, Direction.Right)}>
                    Move Right
                </button>{" "}
                <br />
                <button onClick={() => move(account, Direction.Down)}>
                    Move Down
                </button>
            </div>
        </>
    );
}

export default App;
