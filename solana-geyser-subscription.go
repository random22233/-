package main

import (
	"context"
	"crypto/ed25519"
	"encoding/base64"
	"fmt"
	"io/ioutil"
	"log"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/gagliardetto/solana-go"
	"github.com/gagliardetto/solana-go/rpc"
	"github.com/spf13/viper"
	"google.golang.org/grpc"
	"google.golang.org/grpc/credentials/insecure"
	"google.golang.org/grpc/metadata"

	geyser "github.com/jito-labs/geyser-grpc-plugin/gen/geyser"
)

type Config struct {
	PrivateKey    string `mapstructure:"private_key"`
	RecipientAddr string `mapstructure:"recipient_address"`
	Amount        uint64 `mapstructure:"amount"`
	GeyserURL     string `mapstructure:"geyser_url"`
	APIKey        string `mapstructure:"api_key"`
	RpcURL        string `mapstructure:"rpc_url"`
}

func loadConfig() (*Config, error) {
	viper.SetConfigName("config")
	viper.SetConfigType("yaml")
	viper.AddConfigPath(".")

	if err := viper.ReadInConfig(); err != nil {
		return nil, fmt.Errorf("error reading config file: %w", err)
	}

	var config Config
	if err := viper.Unmarshal(&config); err != nil {
		return nil, fmt.Errorf("unable to decode config into struct: %w", err)
	}

	return &config, nil
}

func main() {
	// Загрузка конфигурации
	config, err := loadConfig()
	if err != nil {
		log.Fatalf("Failed to load configuration: %v", err)
	}

	// Создание контекста с возможностью отмены
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Добавление метаданных для авторизации
	md := metadata.New(map[string]string{
		"x-api-key": config.APIKey,
	})
	ctx = metadata.NewOutgoingContext(ctx, md)

	// Создание соединения gRPC
	conn, err := grpc.Dial(
		config.GeyserURL,
		grpc.WithTransportCredentials(insecure.NewCredentials()),
	)
	if err != nil {
		log.Fatalf("Failed to connect to gRPC server: %v", err)
	}
	defer conn.Close()

	// Создание клиента gRPC
	client := geyser.NewGeyserClient(conn)

	// Подготовка запроса на подписку
	request := &geyser.SubscribeRequest{
		Slots: &geyser.SubscribeRequestSlots{},
	}

	// Подписка на события
	stream, err := client.Subscribe(ctx, request)
	if err != nil {
		log.Fatalf("Failed to subscribe: %v", err)
	}

	// Декодирование приватного ключа
	privateKeyBytes, err := base64.StdEncoding.DecodeString(config.PrivateKey)
	if err != nil {
		log.Fatalf("Failed to decode private key: %v", err)
	}

	// Создание клиента Solana RPC
	solanaClient := rpc.New(config.RpcURL)

	// Создание обработчика сигналов для корректного завершения
	signalCh := make(chan os.Signal, 1)
	signal.Notify(signalCh, syscall.SIGINT, syscall.SIGTERM)

	log.Println("Started listening for new blocks...")

	// Основной цикл обработки событий
	go func() {
		for {
			update, err := stream.Recv()
			if err != nil {
				log.Printf("Error receiving update: %v", err)
				cancel()
				return
			}

			if slotUpdate := update.GetSlot(); slotUpdate != nil {
				slot := slotUpdate.Slot
				log.Printf("New block detected at slot: %d", slot)

				// Отправка транзакции
				err := sendTransaction(solanaClient, privateKeyBytes, config.RecipientAddr, config.Amount)
				if err != nil {
					log.Printf("Failed to send transaction: %v", err)
				} else {
					log.Printf("Transaction sent successfully for block at slot: %d", slot)
				}
			}
		}
	}()

	// Ожидание сигнала завершения
	<-signalCh
	log.Println("Shutting down...")
}

func sendTransaction(client *rpc.Client, privateKeyBytes []byte, recipientAddr string, amount uint64) error {
	// Создание пары ключей из приватного ключа
	privateKey := ed25519.NewKeyFromSeed(privateKeyBytes[:32])
	account := solana.NewAccountFromPrivateKeyBytes(privateKey)

	// Получение последнего блокхеша
	recentBlockhash, err := client.GetRecentBlockhash(context.Background(), rpc.CommitmentFinalized)
	if err != nil {
		return fmt.Errorf("failed to get recent blockhash: %w", err)
	}

	// Парсинг адреса получателя
	recipient, err := solana.PublicKeyFromBase58(recipientAddr)
	if err != nil {
		return fmt.Errorf("invalid recipient address: %w", err)
	}

	// Создание транзакции
	tx, err := solana.NewTransaction(
		[]solana.Instruction{
			solana.NewTransferInstruction(
				amount,
				account.PublicKey(),
				recipient,
			).Build(),
		},
		recentBlockhash.Value.Blockhash,
		solana.TransactionPayer(account.PublicKey()),
	)
	if err != nil {
		return fmt.Errorf("failed to create transaction: %w", err)
	}

	// Подписание транзакции
	_, err = tx.Sign(
		func(key solana.PublicKey) *solana.PrivateKey {
			if account.PublicKey().Equals(key) {
				return &account.PrivateKey
			}
			return nil
		},
	)
	if err != nil {
		return fmt.Errorf("failed to sign transaction: %w", err)
	}

	// Отправка транзакции
	sig, err := client.SendTransactionWithOpts(
		context.Background(),
		tx,
		rpc.TransactionOpts{
			SkipPreflight:       false,
			PreflightCommitment: rpc.CommitmentFinalized,
		},
	)
	if err != nil {
		return fmt.Errorf("failed to send transaction: %w", err)
	}

	log.Printf("Transaction sent with signature: %s", sig.String())
	return nil
}
