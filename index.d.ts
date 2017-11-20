declare	namespace clortho {
		interface MainOptions {
			service: string;
			username?: string;
			message?: string;
			cli?: boolean;
			refresh?: boolean;
		}

		interface Credentials {
			username: string;
			password: string;
		}

		interface ClorthoForService {
			getFromKeyChain(username: string): Promise<Credentials>;

			prompt(username: string, message?: string, cli?: boolean): Promise<Credentials>;

			saveToKeychain (username: string, password: string): Promise<boolean>;

			trySaveToKeychain (credential: Credentials): Promise<Credentials>;

			removeFromKeychain (username: string): Promise<boolean>;

		}

		function forService(service: string): ClorthoForService;
	}

	declare function clortho(options: clortho.MainOptions): Promise<clortho.Credentials>;

	export = clortho;
