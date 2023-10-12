import { WalletAdapterNetwork, WalletError } from '@solana/wallet-adapter-base';
import { ConnectionProvider, WalletProvider } from '@solana/wallet-adapter-react';
import {
  UnsafeBurnerWalletAdapter
} from '@solana/wallet-adapter-wallets';
import { Cluster, clusterApiUrl } from '@solana/web3.js';
import { FC, ReactNode, useCallback, useMemo } from 'react';
import { AutoConnectProvider, useAutoConnect } from './AutoConnectProvider';
import { notify } from "../utils/notifications";
import { NetworkConfigurationProvider, useNetworkConfiguration } from './NetworkConfigurationProvider';
import dynamic from "next/dynamic";

const ReactUIWalletModalProviderDynamic = dynamic(
  async () =>
    (await import("@solana/wallet-adapter-react-ui")).WalletModalProvider,
  { ssr: false }
);

const WalletContextProvider: FC<{ children: ReactNode }> = ({ children }) => {
  const { autoConnect } = useAutoConnect();
  const { networkConfiguration } = useNetworkConfiguration();
  const network = networkConfiguration as WalletAdapterNetwork;
  const endpoint = useMemo(() => {
    // 检查network是否为'localnet'
    if (network === 'localnet') {
      // 如果是'localnet'，则将endpoint设置为'http://127.0.0.1:8899'
      return 'http://127.0.0.1:8899';
    } else {
      // 如果不是'localnet'，则保持原来的设置
      return clusterApiUrl(network);
    }
  }, [network]);  // 确保当network变化时重新计算endpoint
  console.log(network);

  const wallets = useMemo(
    () => [
      new UnsafeBurnerWalletAdapter(),
    ],
    [network]
  );

  const onError = useCallback(
    (error: WalletError) => {
      notify({ type: 'error', message: error.message ? `${error.name}: ${error.message}` : error.name });
      console.error(error);
    },
    []
  );

  return (
    // TODO: updates needed for updating and referencing endpoint: wallet adapter rework
    <ConnectionProvider endpoint={endpoint}>
      <WalletProvider wallets={wallets} onError={onError} autoConnect={autoConnect}>
        <ReactUIWalletModalProviderDynamic>
          {children}
        </ReactUIWalletModalProviderDynamic>
      </WalletProvider>
    </ConnectionProvider>
  );
};

export const ContextProvider: FC<{ children: ReactNode }> = ({ children }) => {
  return (
    <>
      <NetworkConfigurationProvider>
        <AutoConnectProvider>
          <WalletContextProvider>{children}</WalletContextProvider>
        </AutoConnectProvider>
      </NetworkConfigurationProvider>
    </>
  );
};
